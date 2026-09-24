"""Configure an explicitly authorised task Routine or report one occurrence."""
from __future__ import annotations
import argparse
from uuid import UUID
from .client import _client
from .tool_common import run, read_json_object


def app(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(prog="context_routine", description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("read").add_argument("task_id", type=UUID)
    configure = sub.add_parser("configure", help="Save schedule; enabled=true requires confirmed=true and explicit user approval",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog='Example --json: {"expected_revision":2,"schedule":{"timezone":"UTC","local_time":"09:00","weekdays":[1,2,3,4,5]},"enabled":false}\nFor intervals use every_minutes instead of local_time/weekdays. Explicit enable adds "enabled":true,"confirmed":true. Task edits pause the schedule.')
    configure.add_argument("task_id", type=UUID)
    configure.add_argument("--json")
    configure.add_argument("--file")
    result = sub.add_parser("result", help="Report your assigned occurrence; substantive output defaults to review")
    result.add_argument("run_id", type=UUID)
    result.add_argument("--status", choices=["completed", "review", "blocked"], default="review")
    result.add_argument("--result", required=True)
    args = parser.parse_args(argv)
    def action():
        client = _client()
        if args.command == "read":
            return client.routine_read(str(args.task_id))
        if args.command == "configure":
            return client.routine_configure(str(args.task_id), read_json_object(args.json,args.file))
        return client.routine_result(str(args.run_id), args.status, args.result)
    run(action)


def main() -> None:
    app()
