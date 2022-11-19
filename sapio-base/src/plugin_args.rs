// Copyright Judica, Inc 2022
//
// This Source Code Form is subject to the terms of the Mozilla Public
//  License, v. 2.0. If a copy of the MPL was not distributed with this
//  file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! arguments for passing into a sapio module
use crate::effects::MapEffectDB;
use sapio_data_repr::HasSapioModuleSchema;
use serde::{Deserialize, Serialize};

/// a remote derivation for the network definitions
#[derive(Serialize, Deserialize)]
#[serde(remote = "bitcoin::Network")]
pub enum NetworkDef {
    /// Classic Bitcoin
    Bitcoin,
    /// Bitcoin's testnet
    Testnet,
    /// Bitcoin's signet
    Signet,
    /// Bitcoin's regtest
    Regtest,
}

/// # Arguments For Creating this Contract
/// Provide this information to create an instance of a contract
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CreateArgs<S> {
    /// # The Main Contract Arguments
    pub arguments: S,
    /// # Contextual Arguments
    /// Others arguments set by general system settings
    pub context: ContextualArguments,
}
impl<S: HasSapioModuleSchema> HasSapioModuleSchema for CreateArgs<S> {
    fn get_schema() -> sapio_data_repr::SapioModuleSchema {
        todo!()
    }
}

/// # Contextual Arguments For Creating this Contract
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ContextualArguments {
    #[serde(with = "NetworkDef")]
    /// # The Network the contract should be created for.
    pub network: bitcoin::Network,
    #[serde(with = "bitcoin::util::amount::serde::as_sat")]
    /// # The Amount of Funds Available to the Contract as Bitcoin.
    pub amount: bitcoin::util::amount::Amount,

    /// # Effects to augment compilations with
    #[serde(skip_serializing_if = "MapEffectDB::skip_serializing", default)]
    pub effects: MapEffectDB,
}
impl HasSapioModuleSchema for ContextualArguments {
    fn get_schema() -> sapio_data_repr::SapioModuleSchema {
        todo!()
    }
}
