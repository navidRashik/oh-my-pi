import { untilAborted } from "@oh-my-pi/pi-utils";
import { throwIfAborted } from "../tool-errors";

function markHandled<T>(promise: Promise<T>): Promise<T> {
	void promise.catch(() => {});
	return promise;
}

/** Sleeps inside evaluated browser code while honoring cancellation without leaking detached rejections. */
export function waitForBrowserRun(ms: number, signal: AbortSignal): Promise<void> {
	const promise = (async () => {
		throwIfAborted(signal);
		await untilAborted(signal, () => Bun.sleep(ms));
		throwIfAborted(signal);
	})();
	return markHandled(promise);
}

/** Binds a long-lived browser facade to one run and marks detached call rejections as handled. */
export function bindBrowserRunFacade<T extends object>(target: T, signal: AbortSignal): T {
	const cache = new Map<PropertyKey, unknown>();
	return new Proxy(target, {
		get(current, prop) {
			throwIfAborted(signal);
			const cached = cache.get(prop);
			if (cached) return cached;
			const value = Reflect.get(current, prop, current);
			if (typeof value === "function") {
				const wrapped = (...args: unknown[]): unknown => {
					throwIfAborted(signal);
					const result = Reflect.apply(value, current, args);
					if (result && typeof result === "object") {
						const then = Reflect.get(result, "then");
						if (typeof then === "function") {
							const promise = Promise.resolve(result).then(resolved => {
								throwIfAborted(signal);
								return resolved;
							});
							return markHandled(promise);
						}
					}
					throwIfAborted(signal);
					return result;
				};
				cache.set(prop, wrapped);
				return wrapped;
			}
			if (value && typeof value === "object") {
				const wrapped = bindBrowserRunFacade(value, signal);
				cache.set(prop, wrapped);
				return wrapped;
			}
			return value;
		},
	});
}
