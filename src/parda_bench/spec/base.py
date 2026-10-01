from pydantic import BaseModel, ConfigDict


class Strict(BaseModel):
    """Authored data (samples, scenarios, manifests): unknown fields are an error, so a typo cannot pass silently."""

    model_config = ConfigDict(extra="forbid", frozen=True, populate_by_name=True, use_attribute_docstrings=True)


class Lenient(BaseModel):
    """Messages from tools and drivers: extra fields are ignored, so implementations may add their own."""

    model_config = ConfigDict(extra="ignore", frozen=True, populate_by_name=True, use_attribute_docstrings=True)
