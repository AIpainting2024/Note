import { useCallback, useEffect, useState } from "react";
import { api, errorText } from "../lib/api";
import type { Counts, Entry } from "../lib/types";
import { useUI } from "../stores/ui";

/** 某一天的记录；revision 变化（任一处写入）后自动重取 */
export function useDayEntries(day: string) {
  const revision = useUI((s) => s.revision);
  const [entries, setEntries] = useState<Entry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    try {
      const list = await api.listEntries(day);
      setEntries(list);
      setError(null);
    } catch (err) {
      setError(errorText(err));
    } finally {
      setLoading(false);
    }
  }, [day]);

  useEffect(() => {
    void reload();
  }, [reload, revision]);

  return { entries, loading, error, reload };
}

/** 顶栏/待办栏统计 */
export function useCounts() {
  const revision = useUI((s) => s.revision);
  const [counts, setCounts] = useState<Counts | null>(null);

  useEffect(() => {
    let alive = true;
    api
      .getCounts()
      .then((c) => {
        if (alive) setCounts(c);
      })
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, [revision]);

  return counts;
}

/** 月历打点：某月内有记录的日期 */
export function useMarkedDays(start: string, end: string) {
  const revision = useUI((s) => s.revision);
  const [days, setDays] = useState<string[]>([]);

  useEffect(() => {
    let alive = true;
    api
      .listMarkedDays(start, end)
      .then((list) => {
        if (alive) setDays(list);
      })
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, [start, end, revision]);

  return days;
}

/** 日历用：区间内带截止日期的待办（含已完成） */
export function useTodosRange(start: string, end: string) {
  const revision = useUI((s) => s.revision);
  const [todos, setTodos] = useState<Entry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    setLoading(true);
    api
      .listTodosRange(start, end)
      .then((list) => {
        if (!alive) return;
        setTodos(list);
        setError(null);
      })
      .catch((err) => {
        if (alive) setError(errorText(err));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [start, end, revision]);

  return { todos, loading, error };
}
