// Copyright Judica, Inc 2021
//
// This Source Code Form is subject to the terms of the Mozilla Public
//  License, v. 2.0. If a copy of the MPL was not distributed with this
//  file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! general non-parameter compilation state required by all contracts
use super::{Amount, Compilable, CompilationError, Compiled};
use crate::contract::compiler::InternalCompilerTag;

use bitcoin::Network;

use sapio_base::effects::EffectPath;
use sapio_base::effects::PathFragment;
pub use sapio_base::effects::{EffectDB, MapEffectDB};

use sapio_ctv_emulator_trait::CTVEmulator;
use std::convert::TryInto;

use std::collections::HashSet;

use std::sync::Arc;

/// Context is used to track statet during compilation such as remaining value.
pub struct Context {
    /* TODO: Add Context Fields! */
    available_funds: Amount,
    emulator: Arc<dyn CTVEmulator>,
    /// which network is the contract building for?
    pub network: Network,
    path: EffectPath,
    already_derived: HashSet<PathFragment>,
    effects: Arc<MapEffectDB>,
}

impl Context {
    /// create a context instance. Should only happen *once* at the very top
    /// level.
    pub fn new(
        network: Network,
        available_funds: Amount,
        emulator: Arc<dyn CTVEmulator>,
        path: EffectPath,
        effects: Arc<MapEffectDB>,
    ) -> Self {
        Context {
            available_funds,
            emulator,
            network,
            // TODO: Should return Option Self if path is not length > 0
            path,
            already_derived: Default::default(),
            effects,
        }
    }
    /// Get this Context's effect database, for clients
    pub unsafe fn get_effects_internal(&self) -> &Arc<MapEffectDB> {
        &self.effects
    }
    /// Get this Context's effect database
    pub(crate) fn get_effects(&self, _: InternalCompilerTag) -> &Arc<MapEffectDB> {
        &self.effects
    }
    /// Gets this Context's Path, but does not clone (left to caller)
    pub fn path(&self) -> &EffectPath {
        &self.path
    }

    /// Derive a new contextual path
    pub fn derive_str<'a>(&mut self, path: Arc<String>) -> Result<Self, CompilationError> {
        let p: PathFragment = path.try_into()?;
        if matches!(p, PathFragment::Named(_)) {
            self.derive(p)
        } else {
            Err(CompilationError::InvalidPathName)
        }
    }
    /// Derive a new contextual path
    pub fn derive_num<T: Into<u64>>(&mut self, path: T) -> Result<Self, CompilationError> {
        self.derive(PathFragment::Branch(path.into()))
    }
    /// Derive a new contextual path
    pub(crate) fn derive(&mut self, path: PathFragment) -> Result<Self, CompilationError> {
        if self.already_derived.contains(&path) {
            Err(CompilationError::ContexPathAlreadyDerived)
        } else {
            self.already_derived.insert(path.clone());
            let new_path = self.path.push(path);
            Ok(Context {
                available_funds: self.available_funds,
                emulator: self.emulator.clone(),
                path: new_path,
                network: self.network,
                already_derived: Default::default(),
                effects: self.effects.clone(),
            })
        }
    }
    /// Method is unsafe, but may (provably!) be only called from within
    /// compiler.rs where the `InternalCompilerTag` may be generated.
    pub(crate) fn internal_clone(&self, _i: InternalCompilerTag) -> Self {
        Context {
            available_funds: self.available_funds,
            emulator: self.emulator.clone(),
            path: self.path.clone(),
            network: self.network,
            already_derived: self.already_derived.clone(),
            effects: self.effects.clone(),
        }
    }

    /// return the available funds
    pub fn funds(&self) -> Amount {
        self.available_funds
    }

    /// use the context's emulator to get a emulated (or not) clause
    pub fn ctv_emulator(
        &self,
        b: bitcoin::hashes::sha256::Hash,
    ) -> Result<sapio_base::Clause, CompilationError> {
        Ok(self.emulator.get_signer_for(b)?)
    }

    /// Compile the compilable item with this context.
    pub fn compile<A: Compilable>(self, a: A) -> Result<Compiled, CompilationError> {
        a.compile(self)
    }

    // TODO: Fix
    /// return a context with the new amount if amount is smaller or equal to available
    pub fn with_amount(self, amount: Amount) -> Result<Self, CompilationError> {
        if self.available_funds < amount {
            Err(CompilationError::OutOfFunds)
        } else {
            Ok(Context {
                available_funds: amount,
                emulator: self.emulator.clone(),
                path: self.path.clone(),
                network: self.network,
                already_derived: self.already_derived.clone(),
                effects: self.effects.clone(),
            })
        }
    }
    /// decrease the amount available in this context object.
    pub fn spend_amount(mut self, amount: Amount) -> Result<Self, CompilationError> {
        if self.available_funds < amount {
            Err(CompilationError::OutOfFunds)
        } else {
            self.available_funds -= amount;
            Ok(self)
        }
    }

    /// Add funds to the context object (not typically needed)
    pub fn add_amount(mut self, amount: Amount) -> Self {
        self.available_funds += amount;
        self
    }

    /// Get a template builder from this context object
    pub fn template(self) -> crate::template::Builder {
        crate::template::Builder::new(self)
    }
}
