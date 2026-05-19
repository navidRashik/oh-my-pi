import { ToolError } from "./tool-errors";

let fflateModulePromise: Promise<typeof import("fflate")> | undefined;
async function loadFflate(): Promise<typeof import("fflate")> {
	if (!fflateModulePromise) fflateModulePromise = import("fflate");
	return fflateModulePromise;
}

export type ArchiveFormat = "zip" | "tar" | "tar.gz";

export interface ArchivePathCandidate {
	archivePath: string;
	subPath: string;
}

export interface ArchiveNode {
	path: string;
	isDirectory: boolean;
	size: number;
	mtimeMs?: number;
}

export interface ArchiveDirectoryEntry extends ArchiveNode {
	name: string;
}

export interface ExtractedArchiveFile extends ArchiveNode {
	bytes: Uint8Array;
}

interface TarStorage {
	type: "tar";
	file: File;
}

interface ZipStorage {
	type: "zip";
	bytes: Uint8Array;
}

type EntryStorage = TarStorage | ZipStorage;

interface ArchiveIndexEntry extends ArchiveNode {
	storage?: EntryStorage;
}

function normalizeArchiveLookupPath(rawPath?: string): string | undefined {
	if (!rawPath) return "";

	const parts = rawPath.replace(/\\/g, "/").split("/");
	const normalizedParts: string[] = [];
	for (const part of parts) {
		if (!part || part === ".") continue;
		if (part === "..") return undefined;
		normalizedParts.push(part);
	}

	return normalizedParts.join("/");
}

function normalizeArchiveEntryPath(rawPath: string): string | undefined {
	const parts = rawPath.replace(/\\/g, "/").split("/");
	const normalizedParts: string[] = [];
	for (const part of parts) {
		if (!part || part === ".") continue;
		if (part === "..") return undefined;
		normalizedParts.push(part);
	}

	if (normalizedParts.length === 0) return undefined;
	return normalizedParts.join("/");
}

function isArchiveDirectoryName(rawPath: string): boolean {
	return rawPath.endsWith("/") || rawPath.endsWith("\\");
}

function upsertArchiveEntry(map: Map<string, ArchiveIndexEntry>, entry: ArchiveIndexEntry): void {
	const existing = map.get(entry.path);
	if (!existing) {
		map.set(entry.path, entry);
		return;
	}

	if (existing.isDirectory && !entry.isDirectory) {
		map.set(entry.path, entry);
		return;
	}

	if (!existing.isDirectory && entry.isDirectory) {
		return;
	}

	map.set(entry.path, {
		...existing,
		size: existing.size || entry.size,
		mtimeMs: existing.mtimeMs ?? entry.mtimeMs,
		storage: existing.storage ?? entry.storage,
	});
}

function ensureParentDirectories(map: Map<string, ArchiveIndexEntry>): void {
	for (const entry of [...map.values()]) {
		const parts = entry.path.split("/");
		const stop = parts.length - 1;
		for (let index = 1; index <= stop; index++) {
			const dirPath = parts.slice(0, index).join("/");
			if (!dirPath || map.has(dirPath)) continue;
			map.set(dirPath, {
				path: dirPath,
				isDirectory: true,
				size: 0,
			});
		}
	}
}

function getArchiveFormatFromPath(filePath: string): ArchiveFormat | undefined {
	const normalized = filePath.toLowerCase();
	if (normalized.endsWith(".tar.gz") || normalized.endsWith(".tgz")) return "tar.gz";
	if (normalized.endsWith(".tar")) return "tar";
	if (normalized.endsWith(".zip")) return "zip";
	return undefined;
}

async function readTarEntries(bytes: Uint8Array): Promise<ArchiveIndexEntry[]> {
	let archive: Bun.Archive;
	try {
		archive = new Bun.Archive(bytes);
	} catch (error) {
		throw new ToolError(error instanceof Error ? error.message : String(error));
	}

	let files: Map<string, File>;
	try {
		files = await archive.files();
	} catch (error) {
		throw new ToolError(error instanceof Error ? error.message : String(error));
	}

	const entries: ArchiveIndexEntry[] = [];
	for (const [rawPath, file] of files) {
		const normalizedPath = normalizeArchiveEntryPath(rawPath);
		if (!normalizedPath) continue;
		const mtimeMs = file.lastModified > 0 ? file.lastModified : undefined;
		entries.push({
			path: normalizedPath,
			isDirectory: false,
			size: file.size,
			mtimeMs,
			storage: { type: "tar", file },
		});
	}

	return entries;
}

// CP437 (code page 437) upper-half lookup for bytes 0x80–0xFF.
// ZIP's historical default encoding for non-UTF-8 filenames.
const CP437_UPPER =
	"\u00C7\u00FC\u00E9\u00E2\u00E4\u00E0\u00E5\u00E7\u00EA\u00EB\u00E8\u00EF\u00EE\u00EC\u00C4\u00C5" +
	"\u00C9\u00E6\u00C6\u00F4\u00F6\u00F2\u00FB\u00F9\u00FF\u00D6\u00DC\u00A2\u00A3\u00A5\u20A7\u0192" +
	"\u00E1\u00ED\u00F3\u00FA\u00F1\u00D1\u00AA\u00BA\u00BF\u2310\u00AC\u00BD\u00BC\u00A1\u00AB\u00BB" +
	"\u2591\u2592\u2593\u2502\u2524\u2561\u2562\u2556\u2555\u2563\u2551\u2557\u255D\u255C\u255B\u2510" +
	"\u2514\u2534\u252C\u251C\u2500\u253C\u255E\u255F\u255A\u2554\u2569\u2566\u2560\u2550\u256C\u2567" +
	"\u2568\u2564\u2565\u2559\u2558\u2552\u2553\u256B\u256A\u2518\u250C\u2588\u2584\u258C\u2590\u2580" +
	"\u03B1\u00DF\u0393\u03C0\u03A3\u03C3\u00B5\u03C4\u03A6\u0398\u03A9\u03B4\u221E\u03C6\u03B5\u2229" +
	"\u2261\u00B1\u2265\u2264\u2320\u2321\u00F7\u2248\u00B0\u2219\u00B7\u221A\u207F\u00B2\u25A0\u00A0";

function decodeCp437(data: Uint8Array): string {
	let s = "";
	for (let i = 0; i < data.length; i++) {
		const b = data[i];
		s += b < 128 ? String.fromCharCode(b) : CP437_UPPER[b - 128];
	}
	return s;
}

/** CRC32 lookup table (IEEE 802.3, reversed polynomial 0xEDB88320). */
const CRC32_TABLE = (() => {
	const t = new Uint32Array(256);
	for (let n = 0; n < 256; n++) {
		let c = n;
		for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
		t[n] = c;
	}
	return t;
})();

function crc32(data: Uint8Array): number {
	let crc = 0xffffffff;
	for (let i = 0; i < data.length; i++) crc = CRC32_TABLE[(crc ^ data[i]) & 0xff] ^ (crc >>> 8);
	return (crc ^ 0xffffffff) >>> 0;
}

const UTF8_DECODER = new TextDecoder("utf-8");

/**
 * Try to extract the Info-ZIP Unicode Path extra field (tag 0x7075).
 * Returns the decoded Unicode name only when the CRC32 of the original filename
 * bytes matches the stored checksum; returns undefined for absent or invalid fields.
 */
function decodeInfoZipUnicodePath(nameBytes: Uint8Array, extra: Uint8Array): string | undefined {
	const dv = new DataView(extra.buffer, extra.byteOffset, extra.byteLength);
	let pos = 0;
	while (pos + 4 <= extra.length) {
		const tag = dv.getUint16(pos, true);
		const sz = dv.getUint16(pos + 2, true);
		if (tag === 0x7075 && sz >= 5 && pos + 4 + sz <= extra.length) {
			if (extra[pos + 4] === 1) {
				const nameCrc = dv.getUint32(pos + 5, true);
				if (crc32(nameBytes) === nameCrc) {
					return UTF8_DECODER.decode(extra.subarray(pos + 9, pos + 4 + sz));
				}
			}
		}
		pos += 4 + sz;
	}
	return undefined;
}

/**
 * Decode a ZIP central directory filename using the standard priority order:
 * 1. General purpose bit 11 (EFS) set → UTF-8.
 * 2. Info-ZIP Unicode Path extra field 0x7075 with valid CRC32 → UTF-8.
 * 3. Otherwise → CP437 (ZIP historical default).
 */
function decodeZipEntryName(flags: number, nameBytes: Uint8Array, extra: Uint8Array): string {
	if (flags & 0x800) return UTF8_DECODER.decode(nameBytes);
	const unicode = decodeInfoZipUnicodePath(nameBytes, extra);
	if (unicode !== undefined) return unicode;
	return decodeCp437(nameBytes);
}

/** Convert a DOS date+time word pair to a Unix timestamp in ms (UTC). */
function dosDateTimeToMs(timeWord: number, dateWord: number): number | undefined {
	if (dateWord === 0) return undefined;
	const sec = (timeWord & 0x1f) * 2;
	const min = (timeWord >> 5) & 0x3f;
	const hr = (timeWord >> 11) & 0x1f;
	const day = dateWord & 0x1f;
	const mo = ((dateWord >> 5) & 0x0f) - 1;
	const yr = ((dateWord >> 9) & 0x7f) + 1980;
	const ms = Date.UTC(yr, mo, day, hr, min, sec);
	return Number.isNaN(ms) ? undefined : ms;
}

/**
 * Locate the End of Central Directory record by scanning backward from the end
 * of the buffer (accounts for optional ZIP file comment up to 65535 bytes).
 */
function findEOCDOffset(bytes: Uint8Array, dv: DataView): number {
	const lo = Math.max(0, bytes.length - 22 - 65535);
	for (let i = bytes.length - 22; i >= lo; i--) {
		if (dv.getUint32(i, true) === 0x06054b50) {
			if (i + 22 + dv.getUint16(i + 20, true) === bytes.length) return i;
		}
	}
	return -1;
}

async function readZipEntries(bytes: Uint8Array): Promise<ArchiveIndexEntry[]> {
	const { inflateSync } = await loadFflate();

	if (bytes.length < 22) throw new ToolError("Invalid ZIP: file too small");

	const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);

	const eocdOffset = findEOCDOffset(bytes, dv);
	if (eocdOffset < 0) throw new ToolError("Invalid ZIP: EOCD record not found");

	let cdCount = dv.getUint16(eocdOffset + 10, true);
	let cdOffset = dv.getUint32(eocdOffset + 16, true);

	// Check for ZIP64 EOCD locator immediately before the regular EOCD (20-byte record).
	if (eocdOffset >= 20 && dv.getUint32(eocdOffset - 20, true) === 0x07064b50) {
		const z64EocdOff = Number(dv.getBigUint64(eocdOffset - 12, true));
		if (z64EocdOff + 56 <= bytes.length && dv.getUint32(z64EocdOff, true) === 0x06064b50) {
			cdCount = Number(dv.getBigUint64(z64EocdOff + 32, true));
			cdOffset = Number(dv.getBigUint64(z64EocdOff + 48, true));
		}
	}

	const entries: ArchiveIndexEntry[] = [];
	let pos = cdOffset;

	for (let i = 0; i < cdCount && pos + 46 <= bytes.length; i++) {
		if (dv.getUint32(pos, true) !== 0x02014b50) break; // Central Directory File Header signature

		const flags = dv.getUint16(pos + 8, true);
		const method = dv.getUint16(pos + 10, true);
		const modTime = dv.getUint16(pos + 12, true);
		const modDate = dv.getUint16(pos + 14, true);
		let compressedSize = dv.getUint32(pos + 20, true);
		let uncompressedSize = dv.getUint32(pos + 24, true);
		const nameLen = dv.getUint16(pos + 28, true);
		const extraLen = dv.getUint16(pos + 30, true);
		const commentLen = dv.getUint16(pos + 32, true);
		let localOffset = dv.getUint32(pos + 42, true);

		const nameStart = pos + 46;
		const nameBytes = bytes.subarray(nameStart, nameStart + nameLen);
		const extraBytes = bytes.subarray(nameStart + nameLen, nameStart + nameLen + extraLen);

		// ZIP64 extended information (tag 0x0001): sizes/offset may overflow 32-bit fields.
		if (uncompressedSize === 0xffffffff || compressedSize === 0xffffffff || localOffset === 0xffffffff) {
			const edv = new DataView(extraBytes.buffer, extraBytes.byteOffset, extraBytes.byteLength);
			let ep = 0;
			while (ep + 4 <= extraBytes.length) {
				const etag = edv.getUint16(ep, true);
				const esz = edv.getUint16(ep + 2, true);
				if (etag === 0x0001) {
					let eo = ep + 4;
					if (uncompressedSize === 0xffffffff && eo + 8 <= ep + 4 + esz) {
						uncompressedSize = Number(edv.getBigUint64(eo, true));
						eo += 8;
					}
					if (compressedSize === 0xffffffff && eo + 8 <= ep + 4 + esz) {
						compressedSize = Number(edv.getBigUint64(eo, true));
						eo += 8;
					}
					if (localOffset === 0xffffffff && eo + 8 <= ep + 4 + esz) {
						localOffset = Number(edv.getBigUint64(eo, true));
					}
					break;
				}
				ep += 4 + esz;
			}
		}

		pos += 46 + nameLen + extraLen + commentLen;

		const rawPath = decodeZipEntryName(flags, nameBytes, extraBytes);
		const normalizedPath = normalizeArchiveEntryPath(rawPath);
		if (!normalizedPath) continue;

		const isDirectory = isArchiveDirectoryName(rawPath);
		const mtimeMs = dosDateTimeToMs(modTime, modDate);

		if (isDirectory) {
			entries.push({ path: normalizedPath, isDirectory: true, size: 0, mtimeMs });
			continue;
		}

		// Read the local file header to find where the compressed data starts.
		if (localOffset + 30 > bytes.length || dv.getUint32(localOffset, true) !== 0x04034b50) {
			throw new ToolError(`Invalid local file header at offset ${localOffset} for '${normalizedPath}'`);
		}
		const lfhNameLen = dv.getUint16(localOffset + 26, true);
		const lfhExtraLen = dv.getUint16(localOffset + 28, true);
		const dataStart = localOffset + 30 + lfhNameLen + lfhExtraLen;
		const compressedData = bytes.subarray(dataStart, dataStart + compressedSize);

		let fileBytes: Uint8Array;
		if (method === 0) {
			fileBytes = compressedData.slice(); // stored; copy to avoid pinning the full ZIP buffer
		} else if (method === 8) {
			try {
				fileBytes = inflateSync(compressedData, {
					out: uncompressedSize > 0 ? new Uint8Array(uncompressedSize) : undefined,
				});
			} catch (error) {
				throw new ToolError(
					`Failed to decompress '${normalizedPath}': ${error instanceof Error ? error.message : String(error)}`,
				);
			}
		} else {
			throw new ToolError(`Unsupported ZIP compression method ${method} for '${normalizedPath}'`);
		}

		entries.push({
			path: normalizedPath,
			isDirectory: false,
			size: uncompressedSize || fileBytes.byteLength,
			mtimeMs,
			storage: { type: "zip", bytes: fileBytes },
		});
	}

	return entries;
}

export function parseArchivePathCandidates(filePath: string): ArchivePathCandidate[] {
	const normalized = filePath.replace(/\\/g, "/");
	const pattern = /\.(?:tar\.gz|tgz|zip|tar)(?=(?::|$))/gi;
	const seen = new Set<string>();
	const candidates: ArchivePathCandidate[] = [];

	let match: RegExpExecArray | null;
	while (true) {
		match = pattern.exec(normalized);
		if (match === null) {
			break;
		}
		const end = match.index + match[0].length;
		const archivePath = filePath.slice(0, end);
		const subPath = normalized.slice(end).replace(/^:+/, "");
		const key = `${archivePath}\0${subPath}`;
		if (seen.has(key)) continue;
		seen.add(key);
		candidates.push({ archivePath, subPath });
	}

	return candidates.sort((left, right) => right.archivePath.length - left.archivePath.length);
}

export class ArchiveReader {
	readonly format: ArchiveFormat;
	#entries = new Map<string, ArchiveIndexEntry>();

	constructor(format: ArchiveFormat, entries: ArchiveIndexEntry[]) {
		this.format = format;
		for (const entry of entries) {
			upsertArchiveEntry(this.#entries, entry);
		}
		ensureParentDirectories(this.#entries);
	}

	getNode(subPath?: string): ArchiveNode | undefined {
		const normalizedPath = normalizeArchiveLookupPath(subPath);
		if (normalizedPath === undefined) return undefined;
		if (normalizedPath === "") {
			return { path: "", isDirectory: true, size: 0 };
		}

		const entry = this.#entries.get(normalizedPath);
		if (!entry) return undefined;
		return {
			path: entry.path,
			isDirectory: entry.isDirectory,
			size: entry.size,
			mtimeMs: entry.mtimeMs,
		};
	}

	listDirectory(subPath?: string): ArchiveDirectoryEntry[] {
		const normalizedPath = normalizeArchiveLookupPath(subPath);
		if (normalizedPath === undefined) {
			throw new ToolError("Archive path cannot contain '..'");
		}

		if (normalizedPath) {
			const entry = this.#entries.get(normalizedPath);
			if (!entry) {
				throw new ToolError(`Archive path '${normalizedPath}' not found`);
			}
			if (!entry.isDirectory) {
				throw new ToolError(`Archive path '${normalizedPath}' is not a directory`);
			}
		}

		const prefix = normalizedPath ? `${normalizedPath}/` : "";
		const children = new Map<string, ArchiveDirectoryEntry>();

		for (const entry of this.#entries.values()) {
			if (normalizedPath) {
				if (!entry.path.startsWith(prefix) || entry.path === normalizedPath) continue;
			}

			const relativePath = normalizedPath ? entry.path.slice(prefix.length) : entry.path;
			const nextSegment = relativePath.split("/")[0];
			if (!nextSegment) continue;

			const childPath = normalizedPath ? `${normalizedPath}/${nextSegment}` : nextSegment;
			if (children.has(childPath)) continue;

			const childEntry = this.#entries.get(childPath);
			const isDirectory = childEntry?.isDirectory ?? relativePath.includes("/");
			children.set(childPath, {
				name: nextSegment,
				path: childPath,
				isDirectory,
				size: isDirectory ? 0 : (childEntry?.size ?? entry.size),
				mtimeMs: childEntry?.mtimeMs ?? entry.mtimeMs,
			});
		}

		return [...children.values()].sort((left, right) =>
			left.name.toLowerCase().localeCompare(right.name.toLowerCase()),
		);
	}

	async readFile(subPath: string): Promise<ExtractedArchiveFile> {
		const normalizedPath = normalizeArchiveLookupPath(subPath);
		if (!normalizedPath) {
			throw new ToolError("Archive file path is required");
		}

		const entry = this.#entries.get(normalizedPath);
		if (!entry) {
			throw new ToolError(`Archive file '${normalizedPath}' not found`);
		}
		if (entry.isDirectory) {
			throw new ToolError(`Archive path '${normalizedPath}' is a directory`);
		}
		if (!entry.storage) {
			throw new ToolError(`Archive file '${normalizedPath}' has no readable storage`);
		}

		const bytes = entry.storage.type === "tar" ? await entry.storage.file.bytes() : entry.storage.bytes;

		return {
			path: entry.path,
			isDirectory: false,
			size: entry.size,
			mtimeMs: entry.mtimeMs,
			bytes,
		};
	}
}

export async function openArchive(filePath: string): Promise<ArchiveReader> {
	const format = getArchiveFormatFromPath(filePath);
	if (!format) {
		throw new ToolError(`Unsupported archive format: ${filePath}`);
	}

	const bytes = await Bun.file(filePath).bytes();
	const entries = format === "zip" ? await readZipEntries(bytes) : await readTarEntries(bytes);
	return new ArchiveReader(format, entries);
}
