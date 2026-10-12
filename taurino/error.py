"""Public errors and protocol exceptions."""


class TaurinoError(Exception):
    """Base error for this package."""


class StateAlreadyExistsError(TaurinoError):
    pass


class StateNotFoundError(TaurinoError):
    pass


class ProtocolError(TaurinoError):
    pass


class MultiplexerClosedError(TaurinoError):
    pass


class RequestIdsExhausted(TaurinoError):
    pass


class RuntimeStartError(TaurinoError):
    pass


class RuntimeStateError(TaurinoError):
    pass


class AppLifecycleError(TaurinoError):
    pass
