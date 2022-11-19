// Copyright Judica, Inc 2021
//
// This Source Code Form is subject to the terms of the Mozilla Public
//  License, v. 2.0. If a copy of the MPL was not distributed with this
//  file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Functionality for working with ranges of amounts
use bitcoin::util::amount::Amount;

use serde::{Deserialize, Serialize};

/// A wrapper around `bitcoin::Amount` to force it to serialize with f64.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Ord, PartialOrd, PartialEq, Eq)]
#[serde(transparent)]
pub struct AmountF64(
    /// # Amount (BTC)
    // #[schemars(with = "f64")]
    #[serde(with = "bitcoin::util::amount::serde::as_btc")]
    Amount,
);

impl From<Amount> for AmountF64 {
    fn from(a: Amount) -> AmountF64 {
        AmountF64(a)
    }
}
impl From<AmountF64> for Amount {
    fn from(a: AmountF64) -> Amount {
        a.0
    }
}
/// A wrapper around `bitcoin::Amount` to force it to serialize with u64.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Ord, PartialOrd, PartialEq, Eq)]
#[serde(transparent)]
pub struct AmountU64(
    /// # Amount (Sats)
    // #[schemars(with = "u64")]
    #[serde(with = "bitcoin::util::amount::serde::as_sat")]
    Amount,
);

impl From<Amount> for AmountU64 {
    fn from(a: Amount) -> AmountU64 {
        AmountU64(a)
    }
}
impl From<u64> for AmountU64 {
    fn from(a: u64) -> Self {
        AmountU64(Amount::from_sat(a))
    }
}
impl From<AmountU64> for Amount {
    fn from(a: AmountU64) -> Amount {
        a.0
    }
}
impl From<AmountU64> for u64 {
    fn from(a: AmountU64) -> u64 {
        a.0.as_sat()
    }
}
/// `AmountRange` makes it simple to track and update the range of allowed values
/// for a contract to receive.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct AmountRange {
    #[serde(rename = "min_btc", skip_serializing_if = "Option::is_none", default)]
    min: Option<AmountF64>,
    #[serde(rename = "max_btc", skip_serializing_if = "Option::is_none", default)]
    max: Option<AmountF64>,
}
impl AmountRange {
    /// create a new AmountRange with no set values
    pub fn new() -> AmountRange {
        AmountRange {
            min: None,
            max: None,
        }
    }
    /// Update the min and the max value.
    pub fn update_range(&mut self, amount: Amount) {
        self.min = std::cmp::min(self.min, Some(amount.into()));
        self.max = std::cmp::max(self.max, Some(amount.into()));
    }
    /// Retreive the max value, if set, or return `Amount::min_value`.
    pub fn max(&self) -> Amount {
        self.max.unwrap_or(Amount::min_value().into()).0
    }
}
