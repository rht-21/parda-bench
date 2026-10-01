use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The closed set of entity types Parda Bench labels and scores.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EntityType {
    Aadhaar,
    AadhaarVid,
    Pan,
    Gstin,
    Ifsc,
    UpiId,
    Phone,
    Email,
    PaymentCard,
    Passport,
    VoterId,
    VehicleRegistration,
    PersonName,
    Address,
    DateOfBirth,
}
