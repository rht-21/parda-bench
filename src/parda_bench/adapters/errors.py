class AdapterError(Exception):
    """A tool could not be started or reached, or broke the adapter protocol."""


class ToolError(AdapterError):
    """The tool answered with `status: error`; the worker itself is still healthy."""


class TransportError(AdapterError):
    """The worker crashed, hung or sent garbage; it should be restarted before the next request."""


class CapabilityMismatch(AdapterError):
    pass
