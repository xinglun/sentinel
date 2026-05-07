#!/usr/bin/env python3
"""AI 脚手架の構造化観測モジュール。

Flutter seven_app の core/observability パターンを Python に移植し、
各 AI script に統一的なイベント記録機能を提供する。

設計原則:
- Python 標準ライブラリのみ使用。
- 既存の print 出力は維持し、Sink 失敗は無視する。
- report-only: script の exit code に影響しない。
"""

from __future__ import annotations

import json
import os
import sys
import time
from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone
from enum import Enum
from pathlib import Path
from typing import Any, Protocol


PROJECT_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_LOG_PATH = PROJECT_ROOT / "target" / "ai_observability.jsonl"


# ---------------------------------------------------------------------------
# イベント種別
# ---------------------------------------------------------------------------

class AiEventType(str, Enum):
    """AI 観測イベントの種別。"""

    WORK_ITEM_STARTED = "work_item_started"
    WORK_ITEM_FINISHED = "work_item_finished"
    CHECK_STARTED = "check_started"
    CHECK_PASSED = "check_passed"
    CHECK_FAILED = "check_failed"
    GUARD_VIOLATION = "guard_violation"
    STATUS_GENERATED = "status_generated"


class AiEventLevel(str, Enum):
    """イベントの重要度。"""

    DEBUG = "debug"
    INFO = "info"
    WARNING = "warning"
    ERROR = "error"


# ---------------------------------------------------------------------------
# 実行コンテキスト（TraceContext 相当）
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class AiRunContext:
    """AI 実行を一意に識別するコンテキスト。

    Flutter の TraceContext に相当。
    workItemId + runId で因果関係を追跡する。
    """

    work_item_id: str
    run_id: str

    @classmethod
    def create(cls, work_item_id: str) -> AiRunContext:
        """新しい runId を生成してコンテキストを作成する。"""
        run_id = f"{int(time.time() * 1000):x}"
        return cls(work_item_id=work_item_id, run_id=run_id)

    @classmethod
    def from_env(cls) -> AiRunContext | None:
        """環境変数からコンテキストを復元する。"""
        work_item_id = os.environ.get("AI_WORK_ITEM_ID")
        run_id = os.environ.get("AI_RUN_ID")
        if work_item_id and run_id:
            return cls(work_item_id=work_item_id, run_id=run_id)
        return None

    def to_env(self) -> dict[str, str]:
        """環境変数に渡す辞書を返す。"""
        return {
            "AI_WORK_ITEM_ID": self.work_item_id,
            "AI_RUN_ID": self.run_id,
        }

    def to_fields(self) -> dict[str, str]:
        """イベント出力用の辞書を返す。"""
        return {
            "workItemId": self.work_item_id,
            "runId": self.run_id,
        }


# ---------------------------------------------------------------------------
# 構造化イベント（ObservabilityEvent 相当）
# ---------------------------------------------------------------------------

@dataclass
class AiEvent:
    """AI 脚手架の構造化観測イベント。

    Flutter の ObservabilityEvent に相当。
    """

    event_type: AiEventType
    level: AiEventLevel
    message: str
    timestamp: str = field(default_factory=lambda: datetime.now(timezone.utc).isoformat())
    context: AiRunContext | None = None
    check_id: str | None = None
    command: str | None = None
    result: str | None = None
    duration_ms: int | None = None
    exit_code: int | None = None
    severity: str | None = None
    path: str | None = None
    detail: str | None = None
    fields: dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        """JSON 出力用の辞書を返す。"""
        data: dict[str, Any] = {
            "timestamp": self.timestamp,
            "eventType": self.event_type.value,
            "level": self.level.value,
            "message": self.message,
        }
        if self.context:
            data.update(self.context.to_fields())
        if self.check_id is not None:
            data["checkId"] = self.check_id
        if self.command is not None:
            data["command"] = self.command
        if self.result is not None:
            data["result"] = self.result
        if self.duration_ms is not None:
            data["durationMs"] = self.duration_ms
        if self.exit_code is not None:
            data["exitCode"] = self.exit_code
        if self.severity is not None:
            data["severity"] = self.severity
        if self.path is not None:
            data["path"] = self.path
        if self.detail is not None:
            data["detail"] = self.detail
        if self.fields:
            data["fields"] = self.fields
        return data


# ---------------------------------------------------------------------------
# Sink インターフェース（ObservabilitySink 相当）
# ---------------------------------------------------------------------------

class AiObservabilitySink(Protocol):
    """観測イベントの出力先。

    Flutter の ObservabilitySink に相当。
    """

    def record(self, event: AiEvent) -> None:
        """イベントを記録する。"""
        ...


# ---------------------------------------------------------------------------
# JSON Lines Sink
# ---------------------------------------------------------------------------

class JsonLinesSink:
    """JSON Lines 形式でファイルに出力する Sink。

    Flutter の AppLoggerObservabilitySink に相当するが、
    出力先をファイル（target/ai_observability.jsonl）にする。
    """

    def __init__(self, path: Path = DEFAULT_LOG_PATH) -> None:
        self._path = path

    def record(self, event: AiEvent) -> None:
        """イベントを JSON Lines としてファイルに追記する。"""
        self._path.parent.mkdir(parents=True, exist_ok=True)
        line = json.dumps(event.to_dict(), ensure_ascii=False, separators=(",", ":"))
        with self._path.open("a", encoding="utf-8") as f:
            f.write(line + "\n")


# ---------------------------------------------------------------------------
# Observability Facade（AppObservability 相当）
# ---------------------------------------------------------------------------

class AiObservability:
    """AI 脚手架の観測イベント入口。

    Flutter の AppObservability に相当。
    複数 Sink に fan-out し、1 つの Sink 失敗が他に影響しない。
    """

    def __init__(
        self,
        *,
        context: AiRunContext | None = None,
        sinks: list[AiObservabilitySink] | None = None,
    ) -> None:
        self._context = context
        self._sinks: list[AiObservabilitySink] = sinks if sinks is not None else [JsonLinesSink()]

    @property
    def context(self) -> AiRunContext | None:
        return self._context

    def record(self, event: AiEvent) -> None:
        """イベントを全 Sink に配信する。Sink 失敗は無視する。"""
        if event.context is None and self._context is not None:
            event.context = self._context
        for sink in self._sinks:
            try:
                sink.record(event)
            except Exception as exc:
                print(
                    f"[observability] sink failed: {type(sink).__name__}: {exc}",
                    file=sys.stderr,
                )

    # -- Convenience methods -----------------------------------------------

    def check_started(
        self,
        *,
        check_id: str,
        command: str | None = None,
    ) -> None:
        """check 開始イベントを emit する。"""
        self.record(AiEvent(
            event_type=AiEventType.CHECK_STARTED,
            level=AiEventLevel.INFO,
            message=f"check started: {check_id}",
            check_id=check_id,
            command=command,
        ))

    def check_passed(
        self,
        *,
        check_id: str,
        command: str | None = None,
        duration_ms: int | None = None,
        fields: dict[str, Any] | None = None,
    ) -> None:
        """check 成功イベントを emit する。"""
        self.record(AiEvent(
            event_type=AiEventType.CHECK_PASSED,
            level=AiEventLevel.INFO,
            message=f"check passed: {check_id}",
            check_id=check_id,
            command=command,
            result="passed",
            duration_ms=duration_ms,
            fields=fields or {},
        ))

    def check_failed(
        self,
        *,
        check_id: str,
        command: str | None = None,
        duration_ms: int | None = None,
        detail: str | None = None,
        fields: dict[str, Any] | None = None,
    ) -> None:
        """check 失敗イベントを emit する。"""
        self.record(AiEvent(
            event_type=AiEventType.CHECK_FAILED,
            level=AiEventLevel.ERROR,
            message=f"check failed: {check_id}",
            check_id=check_id,
            command=command,
            result="failed",
            duration_ms=duration_ms,
            detail=detail,
            fields=fields or {},
        ))

    def guard_violation(
        self,
        *,
        check_id: str,
        severity: str,
        path: str,
        detail: str,
    ) -> None:
        """guard 違反イベントを emit する。"""
        self.record(AiEvent(
            event_type=AiEventType.GUARD_VIOLATION,
            level=AiEventLevel.WARNING if severity == "warning" else AiEventLevel.ERROR,
            message=f"guard violation: {path}",
            check_id=check_id,
            severity=severity,
            path=path,
            detail=detail,
        ))

    def work_item_started(
        self,
        *,
        fields: dict[str, Any] | None = None,
    ) -> None:
        """Work Item 開始イベントを emit する。"""
        work_item_id = self._context.work_item_id if self._context else "unknown"
        self.record(AiEvent(
            event_type=AiEventType.WORK_ITEM_STARTED,
            level=AiEventLevel.INFO,
            message=f"work item started: {work_item_id}",
            fields=fields or {},
        ))

    def work_item_finished(
        self,
        *,
        result: str,
        duration_ms: int | None = None,
        fields: dict[str, Any] | None = None,
    ) -> None:
        """Work Item 完了イベントを emit する。"""
        work_item_id = self._context.work_item_id if self._context else "unknown"
        self.record(AiEvent(
            event_type=AiEventType.WORK_ITEM_FINISHED,
            level=AiEventLevel.INFO if result == "passed" else AiEventLevel.ERROR,
            message=f"work item finished: {work_item_id}",
            result=result,
            duration_ms=duration_ms,
            fields=fields or {},
        ))

    def status_generated(
        self,
        *,
        state: str,
        output_path: str,
        fields: dict[str, Any] | None = None,
    ) -> None:
        """Cockpit status 生成イベントを emit する。"""
        self.record(AiEvent(
            event_type=AiEventType.STATUS_GENERATED,
            level=AiEventLevel.INFO,
            message=f"cockpit status generated: {state}",
            result=state,
            path=output_path,
            fields=fields or {},
        ))


# ---------------------------------------------------------------------------
# ヘルパー
# ---------------------------------------------------------------------------

def create_observability(work_item_id: str | None = None) -> AiObservability:
    """標準の AiObservability インスタンスを作成する。

    環境変数 AI_WORK_ITEM_ID / AI_RUN_ID があればそれを使う。
    なければ work_item_id 引数から新しいコンテキストを作成する。
    work_item_id も未指定なら context なしで動作する。
    """
    context = AiRunContext.from_env()
    if context is None and work_item_id:
        context = AiRunContext.create(work_item_id)
    return AiObservability(context=context)


def elapsed_ms(start: float) -> int:
    """開始時刻からの経過ミリ秒を返す。"""
    return int((time.time() - start) * 1000)
