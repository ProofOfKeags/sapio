// Copyright Judica, Inc 2021
//
// This Source Code Form is subject to the terms of the Mozilla Public
//  License, v. 2.0. If a copy of the MPL was not distributed with this
//  file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Formats for Sapio Studio
use crate::contract::abi::continuation::ContinuationPoint;
use crate::contract::object::ObjectMetadata;
use crate::template::output::OutputMeta;
use crate::template::TemplateMetadata;
use ::miniscript::*;
use bitcoin::consensus::serialize;
use bitcoin::util::psbt::PartiallySignedTransaction;
use bitcoin::OutPoint;
use sapio_base::effects::EffectPath;
use sapio_base::serialization_helpers::SArc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Container for data from  `Object::bind_psbt`.
#[derive(Serialize, Deserialize)]
#[serde(rename = "linked_psbt")]
pub struct LinkedPSBT {
    /// a PSBT
    pub psbt: PartiallySignedTransaction,
    /// tx level metadata
    pub metadata: TemplateMetadata,
    /// output specific metadata
    pub output_metadata: Vec<ObjectMetadata>,
    /// added metadata
    pub added_output_metadata: Vec<OutputMeta>,
}

/// Format for a Linked PSBT in Sapio Studio
#[derive(Serialize, Deserialize, JsonSchema, Debug)]
pub enum SapioStudioFormat {
    /// Used for PSBT Return Values
    #[serde(rename = "linked_psbt")]
    LinkedPSBT {
        /// Base 64 Encoded PSBT
        psbt: String,
        /// Hex encoded TXN
        hex: String,
        /// tx level metadata
        metadata: TemplateMetadata,
        /// output specific metadata
        output_metadata: Vec<ObjectMetadata>,
        /// added metadata
        added_output_metadata: Vec<OutputMeta>,
    },
}

impl From<LinkedPSBT> for SapioStudioFormat {
    fn from(l: LinkedPSBT) -> SapioStudioFormat {
        let psbt = {
            let bytes = serialize(&l.psbt);
            base64::encode(bytes)
        };
        let hex = bitcoin::consensus::encode::serialize_hex(&l.psbt.extract_tx());
        SapioStudioFormat::LinkedPSBT {
            psbt,
            hex,
            metadata: l.metadata,
            output_metadata: l.output_metadata,
            added_output_metadata: l.added_output_metadata,
        }
    }
}

/// A `Program` is a wrapper type for a list of
/// JSON objects that should be of form:
/// ```json
/// {
///     "hex" : Hex Encoded Transaction
///     "color" : HTML Color,
///     "metadata" : JSON Value,
///     "utxo_metadata" : {
///         "key" : "value",
///         ...
///     }
/// }
/// ```
#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct Program {
    /// program contains the list of SapioStudio PSBTs
    pub program: BTreeMap<EffectPath, SapioStudioObject>,
}

/// A `SapioStudioObject` is a json-friendly format for a `Object` for use in Sapio Studio
#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct SapioStudioObject {
    /// The object's metadata
    pub metadata: ObjectMetadata,
    /// The main covenant OutPoint
    pub out: OutPoint,
    /// List of SapioStudioFormat PSBTs
    pub txs: Vec<SapioStudioFormat>,
    /// List of continue APIs from this point.
    pub continue_apis: BTreeMap<EffectPath, ContinuationPoint>,
}
