// Copyright Judica, Inc 2021
//
// This Source Code Form is subject to the terms of the Mozilla Public
//  License, v. 2.0. If a copy of the MPL was not distributed with this
//  file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! a function type which is used to wrap a next transaction.
use super::CompilationError;
use super::Context;
use super::TxTmplIt;
use crate::contract::actions::ConditionallyCompileIfList;
use crate::contract::actions::GuardList;
use crate::template::Template;
use sapio_base::effects::EffectDBError;
use sapio_base::simp::ContinuationPointLT;
use sapio_base::simp::SIMPAttachableAt;
use sapio_base::Clause;
use sapio_data_repr::SapioModuleSchema;

use core::marker::PhantomData;

use serde::Deserialize;
use std::sync::Arc;

/// A function which by default finishes, but may receive some context object which can induce the
/// generation of additional transactions (as a suggestion)
pub struct FinishOrFunc<'a, ContractSelf, StatefulArguments, SpecificArgs, WebAPIStatus> {
    /// An (optional) function which returns a Vec of SIMPs to attach to the FinishOrFunc associated
    /// with this continuation point.
    pub simp_gen: Option<
        fn(
            &ContractSelf,
            Context,
            // TODO: Should this be able to observe all/any effects?
        )
            -> Result<Vec<Box<dyn SIMPAttachableAt<ContinuationPointLT>>>, CompilationError>,
    >,
    /// StatefulArgs is needed to capture a general API for all calls, but SpecificArgs is required
    /// for a given function.
    pub coerce_args: fn(StatefulArguments) -> Result<SpecificArgs, CompilationError>,
    /// Guards returns Clauses -- if any -- before the coins should be unlocked
    pub guard: GuardList<'a, ContractSelf>,
    /// conditional_compile_if returns ConditionallyCompileType to determine if a function
    /// should be included.
    pub conditional_compile_if: ConditionallyCompileIfList<'a, ContractSelf>,
    /// func returns an iterator of possible transactions
    /// Implementors should aim to return as few `TxTmpl`s as possible for enhanced
    /// semantics, preferring to split across multiple `FinishOrFunc`'s.
    /// These `TxTmpl`s are non-binding, merely suggested.
    pub func: fn(&ContractSelf, Context, SpecificArgs) -> TxTmplIt,
    /// to be filled in if SpecificArgs has a schema, which it might not.
    /// because negative trait bounds do not exists, that is up to the
    /// implementation to decide if the trait exists.
    pub schema: Option<Arc<SapioModuleSchema>>,
    /// name derived from Function Name.
    /// N.B. must be renamable by changing this field!
    pub name: Arc<String>,
    /// Type switch to enable/disable compilation with serialized fields
    /// (if negative trait bounds, could remove!)
    pub f: PhantomData<WebAPIStatus>,
    /// if txtmpls returned by the func should modify guards.
    pub returned_txtmpls_modify_guards: bool,
    /// extract a clause from the txtmpl
    pub extract_clause_from_txtmpl:
        fn(&Template, &Context) -> Result<Option<Clause>, CompilationError>,
}

/// This trait hides the generic parameter `SpecificArgs` in FinishOrFunc
/// through a trait object interface which enables FinishOrFuncs to have a
/// custom type per fucntion, so long as there is a way to convert from
/// StatefulArguments to SpecificArgs via coerce_args. By default, this is
/// presently done through `std::convert::TryInto::try_into`.
pub trait CallableAsFoF<ContractSelf, StatefulArguments> {
    /// Calls the internal function, should convert `StatefulArguments` to `SpecificArgs`.
    fn call(&self, cself: &ContractSelf, ctx: Context, o: StatefulArguments) -> TxTmplIt;

    /// generate any SIMPs to attach here
    fn gen_simps(
        &self,
        cself: &ContractSelf,
        ctx: Context,
    ) -> Result<Vec<Box<dyn SIMPAttachableAt<ContinuationPointLT>>>, CompilationError>;
    /// Calls the internal function, should convert `StatefulArguments` to `SpecificArgs`.
    fn call_data_repr(
        &self,
        _cself: &ContractSelf,
        _ctx: Context,
        _o: sapio_data_repr::SapioModuleBoundaryRepr,
    ) -> TxTmplIt {
        Err(CompilationError::WebAPIDisabled)
    }
    /// to be set to true if call_json may return a non-error type.
    fn web_api(&self) -> bool {
        false
    }
    /// Getter Method for internal field
    fn get_conditional_compile_if(&self) -> ConditionallyCompileIfList<'_, ContractSelf>;
    /// Getter Method for internal field
    fn get_guard(&self) -> GuardList<'_, ContractSelf>;
    /// Get the name for this function
    fn get_name(&self) -> &Arc<String>;
    /// Get the RootSchema for calling this with an update
    fn get_schema(&self) -> &Option<Arc<SapioModuleSchema>>;
    /// get if txtmpls returned by the func should modify guards.
    fn get_returned_txtmpls_modify_guards(&self) -> bool;
    /// extract a clause from the txtmpl
    fn get_extract_clause_from_txtmpl(
        &self,
    ) -> fn(&Template, &Context) -> Result<Option<Clause>, CompilationError>;
    /// rename this object
    fn rename(&mut self, a: Arc<String>);
}

/// Type Tag for FinishOrFunc Variant
pub struct WebAPIEnabled;
/// Type Tag for FinishOrFunc Variant
pub struct WebAPIDisabled;

impl<ContractSelf, StatefulArguments, SpecificArgs> CallableAsFoF<ContractSelf, StatefulArguments>
    for FinishOrFunc<'_, ContractSelf, StatefulArguments, SpecificArgs, WebAPIDisabled>
{
    fn call(&self, cself: &ContractSelf, ctx: Context, o: StatefulArguments) -> TxTmplIt {
        let args = (self.coerce_args)(o)?;
        (self.func)(cself, ctx, args)
    }
    fn get_conditional_compile_if(&self) -> ConditionallyCompileIfList<'_, ContractSelf> {
        self.conditional_compile_if
    }
    fn get_guard(&self) -> GuardList<'_, ContractSelf> {
        self.guard
    }
    fn get_name(&self) -> &Arc<String> {
        &self.name
    }
    fn get_schema(&self) -> &Option<Arc<SapioModuleSchema>> {
        &self.schema
    }
    fn get_returned_txtmpls_modify_guards(&self) -> bool {
        self.returned_txtmpls_modify_guards
    }
    fn get_extract_clause_from_txtmpl(
        &self,
    ) -> fn(&Template, &Context) -> Result<Option<Clause>, CompilationError> {
        self.extract_clause_from_txtmpl
    }

    fn rename(&mut self, a: Arc<String>) {
        self.name = a;
    }

    fn gen_simps(
        &self,
        cself: &ContractSelf,
        ctx: Context,
    ) -> Result<Vec<Box<dyn SIMPAttachableAt<ContinuationPointLT>>>, CompilationError> {
        self.simp_gen.map(|f| (f)(cself, ctx)).unwrap_or(Ok(vec![]))
    }
}

impl<ContractSelf, StatefulArguments, SpecificArgs> CallableAsFoF<ContractSelf, StatefulArguments>
    for FinishOrFunc<'_, ContractSelf, StatefulArguments, SpecificArgs, WebAPIEnabled>
where
    SpecificArgs: for<'de> Deserialize<'de>,
{
    fn call(&self, cself: &ContractSelf, ctx: Context, o: StatefulArguments) -> TxTmplIt {
        let args = (self.coerce_args)(o)?;
        (self.func)(cself, ctx, args)
    }
    fn call_data_repr(
        &self,
        cself: &ContractSelf,
        ctx: Context,
        o: sapio_data_repr::SapioModuleBoundaryRepr,
    ) -> TxTmplIt {
        sapio_data_repr::from_boundary_repr(o)
            .map_err(EffectDBError::SerializationError)
            .map_err(CompilationError::EffectDBError)
            .and_then(|args| (self.func)(cself, ctx, args))
    }
    fn web_api(&self) -> bool {
        true
    }
    fn get_conditional_compile_if(&self) -> ConditionallyCompileIfList<'_, ContractSelf> {
        self.conditional_compile_if
    }
    fn get_guard(&self) -> GuardList<'_, ContractSelf> {
        self.guard
    }
    fn get_name(&self) -> &Arc<String> {
        &self.name
    }
    fn get_schema(&self) -> &Option<Arc<SapioModuleSchema>> {
        &self.schema
    }
    fn get_returned_txtmpls_modify_guards(&self) -> bool {
        self.returned_txtmpls_modify_guards
    }

    fn get_extract_clause_from_txtmpl(
        &self,
    ) -> fn(&Template, &Context) -> Result<Option<Clause>, CompilationError> {
        self.extract_clause_from_txtmpl
    }

    fn rename(&mut self, a: Arc<String>) {
        self.name = a;
    }

    fn gen_simps(
        &self,
        cself: &ContractSelf,
        ctx: Context,
    ) -> Result<Vec<Box<dyn SIMPAttachableAt<ContinuationPointLT>>>, CompilationError> {
        self.simp_gen.map(|f| (f)(cself, ctx)).unwrap_or(Ok(vec![]))
    }
}

/// default clause extractor should not attempt to do anything, but should fail if the txtmpl has attached guards
pub fn default_extract_clause_from_txtmpl(
    t: &Template,
    _ctx: &Context,
) -> Result<Option<Clause>, CompilationError> {
    // Don't return or use the extra guards here
    // because we're within a non-CTV context... if
    // we did, then it would destabilize compilation
    // with effect arguments.
    if !t.guards.is_empty() {
        // N.B.: In theory, the *default* effect
        // could pass up something here.
        // However, we don't do that since there's
        // not much point to it.
        Err(CompilationError::AdditionalGuardsNotAllowedHere)
    } else {
        // Don't add anything...
        Ok(None)
    }
}
