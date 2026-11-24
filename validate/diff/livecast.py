"""No-op stand-in for the private dashboard hook: the tests call status/frame/suite to report progress."""


def status(*args, **kwargs):
    pass


def frame(*args, **kwargs):
    pass


def suite(*args, **kwargs):
    pass
