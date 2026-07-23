"""MoonCoding native micro-app SDK (Python standard library only)."""

import itertools
import json
import sys

PROTOCOL_VERSION = 2
MAX_LINE_BYTES = 64 * 1024


class ProtocolError(RuntimeError):
    pass


class _StopRequested(Exception):
    pass


class MoonCodingApp:
    """JSONL client for logic-only Python micro-apps."""

    def __init__(self):
        self._pending = []
        self._requests = itertools.count(1)
        self._stopped = False
        init = self._read()
        if init.get("type") != "app.init":
            raise ProtocolError("expected app.init")
        if init.get("protocol_version") != PROTOCOL_VERSION:
            raise ProtocolError("unsupported protocol version")
        self.instance_id = init["instance_id"]
        self.app_id = init["app_id"]
        self.capabilities = init.get("capabilities", {})
        self.limits = init.get("limits", {})
        self._write({"type": "app.ready", "protocol_version": PROTOCOL_VERSION})

    def ui_init(self, ui):
        """Send a UI document, or load one from a JSON file path."""
        if isinstance(ui, str):
            with open(ui, "r", encoding="utf-8") as source:
                ui = json.load(source)
        self._write({"type": "ui.init", "ui": ui})

    def ui_patch(self, patch):
        self._write({"type": "ui.patch", "patch": patch})

    def log(self, message, level="info"):
        self._write({"type": "log", "level": str(level), "message": str(message)})

    def error(self, message):
        self._write({"type": "app.error", "message": str(message)})

    def gpio_configure(self, alias, mode):
        self._gpio(alias, "configure", mode=mode)

    def gpio_read(self, alias):
        return bool(self._gpio(alias, "read"))

    def gpio_write(self, alias, value):
        self._gpio(alias, "write", value=bool(value))

    def run(self, on_event, on_stop=None):
        """Dispatch UI events until the host requests a graceful stop."""
        reason = "input closed"
        try:
            while True:
                message = self._next()
                kind = message.get("type")
                if kind == "ui.event":
                    on_event(message.get("event", {}))
                elif kind == "app.stop":
                    reason = message.get("reason", "host requested stop")
                    if on_stop is not None:
                        on_stop()
                    break
                elif kind == "gpio.result":
                    self._pending.append(message)
                else:
                    self.log("ignored host message: " + str(kind), "warn")
        except (EOFError, _StopRequested):
            reason = "host disconnected"
        except Exception as exc:
            reason = "application error"
            self.error(str(exc))
        finally:
            self._send_stopped(reason)

    def _gpio(self, alias, operation, mode=None, value=None):
        request_id = "gpio-{}".format(next(self._requests))
        message = {
            "type": "gpio.request",
            "request_id": request_id,
            "alias": str(alias),
            "operation": operation,
        }
        if mode is not None:
            message["mode"] = mode
        if value is not None:
            message["value"] = value
        self._write(message)

        deferred = []
        while True:
            response = self._read()
            if (
                response.get("type") == "gpio.result"
                and response.get("request_id") == request_id
            ):
                self._pending[0:0] = deferred
                if not response.get("ok"):
                    raise ProtocolError(response.get("error", "GPIO request failed"))
                return response.get("value")
            if response.get("type") == "app.stop":
                self._pending[0:0] = deferred + [response]
                raise _StopRequested()
            deferred.append(response)

    def _next(self):
        if self._pending:
            return self._pending.pop(0)
        return self._read()

    def _read(self):
        raw = sys.stdin.buffer.readline(MAX_LINE_BYTES + 2)
        if not raw:
            raise EOFError()
        if len(raw) > MAX_LINE_BYTES and not raw.endswith(b"\n"):
            while raw and not raw.endswith(b"\n"):
                raw = sys.stdin.buffer.readline(MAX_LINE_BYTES + 2)
            raise ProtocolError("host message exceeds 64KB")
        raw = raw.rstrip(b"\r\n")
        if len(raw) > MAX_LINE_BYTES:
            raise ProtocolError("host message exceeds 64KB")
        try:
            message = json.loads(raw.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise ProtocolError("invalid host JSON: {}".format(exc)) from exc
        if not isinstance(message, dict):
            raise ProtocolError("host message must be a JSON object")
        return message

    def _write(self, message):
        encoded = json.dumps(
            message, ensure_ascii=False, separators=(",", ":")
        ).encode("utf-8")
        if len(encoded) > MAX_LINE_BYTES:
            raise ProtocolError("app message exceeds 64KB")
        sys.stdout.buffer.write(encoded + b"\n")
        sys.stdout.buffer.flush()

    def _send_stopped(self, reason):
        if not self._stopped:
            self._stopped = True
            self._write({"type": "app.stopped", "reason": str(reason)})


App = MoonCodingApp


class HeadlessApp:
    """In-memory stand-in for CLI/button tests (no JSONL host, no real UI)."""

    def __init__(self):
        self.patches = []
        self.logs = []
        self.errors = []
        self.ui = None

    def ui_init(self, ui):
        if isinstance(ui, str):
            with open(ui, "r", encoding="utf-8") as source:
                ui = json.load(source)
        self.ui = ui

    def ui_patch(self, patch):
        self.patches.append(patch)

    def log(self, message, level="info"):
        self.logs.append({"level": str(level), "message": str(message)})

    def error(self, message):
        self.errors.append(str(message))

    def gpio_configure(self, alias, mode):
        return None

    def gpio_read(self, alias):
        return False

    def gpio_write(self, alias, value):
        return None

    def run(self, on_event, on_stop=None):
        raise ProtocolError(
            "HeadlessApp.run is unused; call mooncoding_app.drive_events(handler, events)"
        )


def drive_events(handler, events):
    """Feed synthetic UI events to a handler. Returns nothing; inspect HeadlessApp.patches."""
    for event in events:
        handler(event)


def click(widget_id, **extra):
    """Build a click event matching the native host shape."""
    event = {"id": str(widget_id), "event": "click"}
    event.update(extra)
    return event


def assert_patch(patches, widget_id, **fields):
    """Find the last patch for widget_id and assert field values. Raises AssertionError."""
    matched = [p for p in patches if isinstance(p, dict) and p.get("id") == widget_id]
    if not matched:
        raise AssertionError("no ui_patch for id={!r}; got {!r}".format(widget_id, patches))
    last = matched[-1]
    for key, expected in fields.items():
        actual = last.get(key)
        if actual != expected:
            raise AssertionError(
                "patch id={!r} field {!r}: expected {!r}, got {!r} (patch={!r})".format(
                    widget_id, key, expected, actual, last
                )
            )
    return last

