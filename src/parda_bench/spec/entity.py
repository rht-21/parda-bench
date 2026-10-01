from enum import StrEnum


class EntityType(StrEnum):
    """The closed set of entity types Parda Bench labels and scores."""

    AADHAAR = "AADHAAR"
    AADHAAR_VID = "AADHAAR_VID"
    PAN = "PAN"
    GSTIN = "GSTIN"
    IFSC = "IFSC"
    UPI_ID = "UPI_ID"
    PHONE = "PHONE"
    EMAIL = "EMAIL"
    PAYMENT_CARD = "PAYMENT_CARD"
    PASSPORT = "PASSPORT"
    VOTER_ID = "VOTER_ID"
    VEHICLE_REGISTRATION = "VEHICLE_REGISTRATION"
    PERSON_NAME = "PERSON_NAME"
    ADDRESS = "ADDRESS"
    DATE_OF_BIRTH = "DATE_OF_BIRTH"
