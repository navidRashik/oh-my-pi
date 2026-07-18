/** Platform-specific options for the launch broker and its non-PTY children. */
export interface DaemonSpawnOptions {
	detached: boolean;
	windowsHide?: boolean;
}

/** Hide Windows launch processes while keeping them isolated from the host's console control group. */
export function resolveDaemonSpawnOptions(platform: NodeJS.Platform): DaemonSpawnOptions {
	if (platform !== "win32") return { detached: true };
	return {
		detached: true,
		windowsHide: true,
	};
}
