// Recovered IPC surface from Pluely 1.1.0.
// The frontend calls `re(command, args)` which delegates to Tauri's invoke.

export function invoke<T = unknown>(command: string, args?: Record<string, unknown>): Promise<T> {
  return (window as any).__TAURI_INTERNALS__.invoke(command, args ?? {});
}

export function listen<T = unknown>(event: string, handler: (payload: T) => void): Promise<() => void> {
  return (window as any).__TAURI_INTERNALS__.event.listen(event, (e: { payload: T }) => handler(e.payload));
}
