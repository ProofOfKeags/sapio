// Copyright Judica, Inc 2021
//
// This Source Code Form is subject to the terms of the Mozilla Public
//  License, v. 2.0. If a copy of the MPL was not distributed with this
//  file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! An example of how one might begin building a payment channel contract in Sapio
use bitcoin;
use bitcoin::util::amount::CoinAmount;
use contract::*;

use sapio::*;
use sapio_base::Clause;
use sapio_data_repr;
use sapio_macros::guard;
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};
/// Helper

#[cfg(test)]
mod tests {
    use super::*;
    use ::rand::rngs::OsRng;
    use bitcoin::secp256k1::*;
    use bitcoin::Amount;
    use miniscript::Descriptor;
    use miniscript::DescriptorTrait;

    use sapio_ctv_emulator_trait::CTVAvailable;
    #[test]
    fn it_works() {
        db_serde::register_db("mock".to_string(), |_s| Arc::new(Mutex::new(MockDB {})));
        let full = Secp256k1::new();
        let mut rng = OsRng::new().expect("OsRng");
        let public_keys: Vec<_> = (0..3)
            .map(|_| full.generate_keypair(&mut rng).1.into())
            .collect();
        let resolution = Compiled::from_address(
            Descriptor::<bitcoin::XOnlyPublicKey>::Pkh(miniscript::descriptor::Pkh::new(
                public_keys[2],
            ))
            .address(bitcoin::Network::Regtest)
            .expect("An Address"),
            None,
        );

        let db = Arc::new(Mutex::new(MockDB {}));
        let x: Channel<Start, Args> = Channel {
            pd: PhantomData,
            alice: public_keys[0],
            bob: public_keys[1],
            amount: Amount::from_sat(1).into(),
            resolution: resolution.clone(),
            db: db.clone(),
        };
        let y: Channel<Stop, Args> = Channel {
            pd: PhantomData,
            alice: public_keys[0],
            bob: public_keys[1],
            amount: Amount::from_sat(1).into(),
            resolution,
            db,
        };
        // println!(
        //     "{}",
        //     serde_json::to_string_pretty(&schemars::schema_for!(Channel<Stop, Args>)).unwrap()
        // );
        println!("{}", serde_json::to_string_pretty(&y).unwrap());
        let mut ctx = sapio::contract::Context::new(
            bitcoin::Network::Regtest,
            Amount::from_sat(10000),
            std::sync::Arc::new(CTVAvailable),
            "root".try_into().unwrap(),
            Default::default(),
        );
        Compilable::compile(&x, ctx.derive_str(Arc::new("X".into())).unwrap()).ok();
        Compilable::compile(&y, ctx.derive_str(Arc::new("Y".into())).unwrap()).ok();
    }
}

/// Main Update to Channel
#[derive(Debug, JsonSchema)]
pub struct Update {
    /// hash to revoke
    revoke: bitcoin::hashes::sha256::Hash,
    /// the balances of the channel
    split: (CoinAmount, CoinAmount),
}
impl TryFrom<Args> for Update {
    type Error = CompilationError;
    fn try_from(a: Args) -> Result<Update, CompilationError> {
        if let Args::Update(u) = a {
            Ok(u)
        } else {
            Err(CompilationError::Custom("Unmatched".into()))
        }
    }
}
/// Args are some messages that can be passed to a Channel instance
#[derive(Debug, JsonSchema)]
pub enum Args {
    /// Wrapper around Update
    Update(Update),
    /// Revoke a hash and move to the next state...
    None,
}
impl Default for Args {
    fn default() -> Self {
        Args::None
    }
}
impl StatefulArgumentsTrait for Args {}

/// Handle for DB Types
#[derive(JsonSchema, Serialize, Deserialize)]
pub struct DBHandle {
    type_: String,
    id: String,
}
/// DB Trait is for a Trait Object that can be used to record state updates for a channel.
/// Examples implements a MockDB
pub trait DB {
    /// Simply save a transcript of all messages to reconstrue channel state
    fn save(&self, a: Args);
    /// gets a handle to this DB instance for global lookup
    fn link(&self) -> DBHandle;
}

#[derive(JsonSchema)]
struct MockDB {}
impl DB for MockDB {
    fn save(&self, a: Args) {
        match a {
            Args::Update { .. } => {}
            Args::None => {}
        }
    }
    fn link(&self) -> DBHandle {
        DBHandle {
            type_: "mock".into(),
            id: "".into(),
        }
    }
}

/// Custom Serialization Logic for DB Trait Critically, the method register_db can be used to add
/// resolvers to get references to DB instances of arbitrary types.
mod db_serde {
    use super::*;
    use serde::de::Error;

    use lazy_static::lazy_static;
    lazy_static! {
        static ref DB_TYPES: Mutex<BTreeMap<String, fn(&str) -> Arc<Mutex<dyn DB>>>> =
            Mutex::new(BTreeMap::new());
    }

    pub fn register_db(s: String, f: fn(&str) -> Arc<Mutex<dyn DB>>) {
        assert!(DB_TYPES.lock().unwrap().insert(s, f).is_none());
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Arc<Mutex<dyn DB>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let handle = DBHandle::deserialize(deserializer)?;
        if let Some(f) = DB_TYPES.lock().unwrap().get(&handle.type_) {
            Ok(f(&handle.id))
        } else {
            Err(D::Error::unknown_variant(&handle.type_, &[]))
        }
    }

    pub fn serialize<S>(db: &Arc<Mutex<dyn DB>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        db.lock().unwrap().link().serialize(serializer)
    }
}

/// The Different Operating States a Channel may be in.
/// These States are enum'd at the trait/type level so as
/// to be used as type tags
trait State {}
/// State Start
#[derive(JsonSchema)]
struct Start();
/// state Stop
#[derive(JsonSchema)]
struct Stop();
impl State for Start {}
impl State for Stop {}

#[derive(Serialize, Deserialize)]
struct Channel<T: State, ArgsT: TryInto<Update>> {
    pd: PhantomData<(T, ArgsT)>,
    // TODO: Taproot Fix Encoding
    // #[schemars(with = "bitcoin::hashes::sha256::Hash")]
    alice: bitcoin::XOnlyPublicKey,
    // TODO: Taproot Fix Encoding
    // #[schemars(with = "bitcoin::hashes::sha256::Hash")]
    bob: bitcoin::XOnlyPublicKey,
    amount: CoinAmount,
    resolution: Compiled,
    /// We instruct the JSONSchema to use strings
    // #[schemars(with = "DBHandle")]
    #[serde(with = "db_serde")]
    db: Arc<Mutex<dyn DB>>,
}

fn coerce_args<T>(t: T) -> Result<Update, CompilationError>
where
    T: TryInto<Update, Error = CompilationError>,
{
    t.try_into()
}

/// Functionality Available for a channel regardless of state
impl<T: State> Channel<T, Args>
where
    Self: Contract,
    <Self as Contract>::StatefulArguments: TryInto<Update, Error = CompilationError>,
{
    #[guard]
    fn timeout(self, _ctx: Context) {
        Clause::Older(100)
    }
    #[guard(cached)]
    fn signed(self, _ctx: Context) {
        Clause::And(vec![Clause::Key(self.alice), Clause::Key(self.bob)])
    }

    #[continuation(guarded_by = "[Self::signed]", coerce_args = "coerce_args")]
    fn update_state_a(self, _ctx: sapio::Context, _o: Update) {
        Ok(Box::new(std::iter::empty()))
    }
    #[continuation(guarded_by = "[Self::signed]", coerce_args = "coerce_args")]
    fn update_state_b(self, _ctx: sapio::Context, _o: Update) {
        Ok(Box::new(std::iter::empty()))
    }
    #[continuation(guarded_by = "[Self::signed]", coerce_args = "coerce_args")]
    fn cooperate(self, _ctx: sapio::Context, _o: Update) {
        Ok(Box::new(std::iter::empty()))
    }
}

/// Functionality that differs depending on current State
trait FunctionalityAtState
where
    Self: Sized + Contract,
    <Self as Contract>::StatefulArguments: TryInto<Update>,
{
    decl_then! {begin_contest}
    decl_then! {finish_contest}
}

/// Override begin_contest when state = Start
impl FunctionalityAtState for Channel<Start, Args> {
    #[then]
    fn begin_contest(self, ctx: sapio::Context) {
        ctx.template()
            .add_output(
                self.amount.try_into()?,
                &Channel::<Stop, Args> {
                    pd: Default::default(),
                    alice: self.alice,
                    bob: self.bob,
                    amount: self.amount.try_into().unwrap(),
                    resolution: self.resolution.clone(),
                    db: self.db.clone(),
                },
                None,
            )?
            .into()
    }
}

/// Override finish_contest when state = Start
impl FunctionalityAtState for Channel<Stop, Args> {
    #[then(guarded_by = "[Self::timeout]")]
    fn finish_contest(self, ctx: sapio::Context) {
        ctx.template()
            .add_output(self.amount.try_into()?, &self.resolution, None)?
            .into()
    }
}

/// Implement Contract for Channel<T> and functionality will be correctly assembled for different
/// States.
impl Contract for Channel<Start, Args> {
    declare! {then, Self::begin_contest, Self::finish_contest}
    declare! {updatable<Args>, Self::update_state_a, Self::update_state_b }
    declare! {finish, Self::signed}
}

impl Contract for Channel<Stop, Args> {
    declare! {then, Self::begin_contest, Self::finish_contest}
    declare! {updatable<Args>, Self::update_state_a, Self::update_state_b }
    declare! {finish, Self::signed}
}
