import { createContext, useContext, type PropsWithChildren } from 'react';

export type OperatingSystem = 'browser' | 'linux' | 'macOS' | 'windows' | 'unknown';

// Platform represents the underlying native layer the app is running on.
// This could be Tauri or web.
export type Platform = {
	platform: 'web' | 'tauri'; // This represents the specific platform implementation
	getThumbnailUrlByThumbKey: (thumbKey: string[]) => string;
	getFileUrl: (libraryId: string, locationLocalId: number, filePathId: number) => string;
	openLink: (url: string) => void;
	// Tauri patches `window.confirm` to return `Promise` not `bool`
	confirm(msg: string, cb: (result: boolean) => void): void;
	getOs?(): Promise<OperatingSystem>;
	openDirectoryPickerDialog?(): Promise<null | string | string[]>;
	openFilePickerDialog?(): Promise<null | string | string[]>;
	saveFilePickerDialog?(opts?: { title?: string; defaultPath?: string }): Promise<string | null>;
	showDevtools?(): void;
	openPath?(path: string): void;
	openLogsDir?(): void;
	userHomeDir?(): Promise<string>;
	// Opens a file path with a given ID
	openFilePaths?(library: string, ids: number[]): any;
	revealItems?(
		library: string,
		items: ({ Location: { id: number } } | { FilePath: { id: number } })[]
	): Promise<unknown>;
	getFilePathOpenWithApps?(library: string, ids: number[]): Promise<unknown>;
	openFilePathWith?(library: string, fileIdsAndAppUrls: [number, string][]): Promise<unknown>;
	lockAppTheme?(themeType: 'Auto' | 'Light' | 'Dark'): any;
	auth: {
		start(key: string): any;
		finish?(ret: any): void;
	};
};

// Keep this private and use through helpers below
const context = createContext<Platform>(undefined!);

// is a hook which allows you to fetch information about the current platform from the React context.
export function usePlatform(): Platform {
	const ctx = useContext(context);
	if (!ctx)
		throw new Error(
			"The 'PlatformProvider' has not been mounted above the current 'usePlatform' call."
		);

	return ctx;
}

// provides the platform context to the rest of the app through React context.
// Mount it near the top of your component tree.
export function PlatformProvider({
	platform,
	children
}: PropsWithChildren<{ platform: Platform }>) {
	return <context.Provider value={platform}>{children}</context.Provider>;
}
