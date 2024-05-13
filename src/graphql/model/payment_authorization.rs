use serde::{Deserialize, Serialize};

use super::super::mutation_input_structs::PaymentAuthorizationInput;

/// Payment authorization data that can be attached to order.
///
/// This datatype can be extended with different payment authorization formats.
/// The conversion implementation needs to be adapted accordingly.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub enum PaymentAuthorization {
    /// CVC/CVV number of 3-4 digits.
    CVC(u16),
}

impl From<PaymentAuthorizationInput> for Option<PaymentAuthorization> {
    fn from(value: PaymentAuthorizationInput) -> Self {
        match value.cvc {
            Some(definitely_cvc) => Some(PaymentAuthorization::CVC(definitely_cvc)),
            None => None,
        }
    }
}
