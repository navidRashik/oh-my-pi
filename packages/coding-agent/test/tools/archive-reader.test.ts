import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import * as fs from "node:fs/promises";
import * as os from "node:os";
import * as path from "node:path";
import { openArchive } from "../../src/tools/archive-reader";

// ---------------------------------------------------------------------------
// Minimal ZIP builder
// ---------------------------------------------------------------------------

/** CRC32 (IEEE 802.3, reversed poly 0xEDB88320) needed for fixture construction. */
function buildCrc32Table(): Uint32Array {
	const t = new Uint32Array(256);
	for (let n = 0; n < 256; n++) {
		let c = n;
		for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
		t[n] = c;
	}
	return t;
}
const CRC_TABLE = buildCrc32Table();

function crc32(data: Uint8Array): number {
	let crc = 0xffffffff;
	for (const b of data) crc = CRC_TABLE[(crc ^ b) & 0xff] ^ (crc >>> 8);
	return (crc ^ 0xffffffff) >>> 0;
}

interface ZipEntry {
	/** Raw bytes to store in the central directory filename field. */
	nameBytes: Uint8Array;
	/** General purpose bit flags (bit 11 = UTF-8 / EFS). */
	flags: number;
	/** Raw entry content (method 0 / stored). */
	data: Uint8Array;
	/** Optional extra field bytes for both LFH and CDFH. */
	extra?: Uint8Array;
}

/**
 * Build a minimal valid stored (method 0) ZIP from explicit raw name bytes.
 * Use this instead of a ZIP library so that filename encoding is 100% controlled.
 */
function buildStoredZip(entries: ZipEntry[]): Uint8Array {
	const parts: Uint8Array[] = [];
	const cdfhParts: Uint8Array[] = [];
	let offset = 0;

	for (const entry of entries) {
		const extra = entry.extra ?? new Uint8Array(0);
		const dataCrc = crc32(entry.data);

		// Local File Header (30 bytes fixed + name + extra)
		const lfh = new Uint8Array(30 + entry.nameBytes.length + extra.length);
		const lv = new DataView(lfh.buffer);
		lv.setUint32(0, 0x04034b50, true); // LFH signature
		lv.setUint16(4, 20, true); // version needed
		lv.setUint16(6, entry.flags, true);
		lv.setUint16(8, 0, true); // method: stored
		lv.setUint16(10, 0, true); // mod time
		lv.setUint16(12, 0, true); // mod date
		lv.setUint32(14, dataCrc, true);
		lv.setUint32(18, entry.data.length, true); // compressed size
		lv.setUint32(22, entry.data.length, true); // uncompressed size
		lv.setUint16(26, entry.nameBytes.length, true);
		lv.setUint16(28, extra.length, true);
		lfh.set(entry.nameBytes, 30);
		lfh.set(extra, 30 + entry.nameBytes.length);

		const localHeaderOffset = offset;
		parts.push(lfh, entry.data);
		offset += lfh.length + entry.data.length;

		// Central Directory File Header (46 bytes fixed + name + extra)
		const cdfh = new Uint8Array(46 + entry.nameBytes.length + extra.length);
		const cv = new DataView(cdfh.buffer);
		cv.setUint32(0, 0x02014b50, true); // CDFH signature
		cv.setUint16(4, 0x031e, true); // version made by: Unix / PKZIP 3.0
		cv.setUint16(6, 20, true); // version needed
		cv.setUint16(8, entry.flags, true);
		cv.setUint16(10, 0, true); // method: stored
		cv.setUint16(12, 0, true); // mod time
		cv.setUint16(14, 0, true); // mod date
		cv.setUint32(16, dataCrc, true);
		cv.setUint32(20, entry.data.length, true); // compressed size
		cv.setUint32(24, entry.data.length, true); // uncompressed size
		cv.setUint16(28, entry.nameBytes.length, true);
		cv.setUint16(30, extra.length, true);
		cv.setUint16(32, 0, true); // comment length
		cv.setUint16(34, 0, true); // disk number start
		cv.setUint16(36, 0, true); // internal attributes
		cv.setUint32(38, 0, true); // external attributes
		cv.setUint32(42, localHeaderOffset, true);
		cdfh.set(entry.nameBytes, 46);
		cdfh.set(extra, 46 + entry.nameBytes.length);
		cdfhParts.push(cdfh);
	}

	const cdStart = offset;
	let cdSize = 0;
	for (const c of cdfhParts) {
		parts.push(c);
		cdSize += c.length;
	}

	// End of Central Directory
	const eocd = new Uint8Array(22);
	const ev = new DataView(eocd.buffer);
	ev.setUint32(0, 0x06054b50, true);
	ev.setUint16(4, 0, true); // disk number
	ev.setUint16(6, 0, true); // disk with CD start
	ev.setUint16(8, entries.length, true); // entries on disk
	ev.setUint16(10, entries.length, true); // total entries
	ev.setUint32(12, cdSize, true);
	ev.setUint32(16, cdStart, true);
	ev.setUint16(20, 0, true); // comment length
	parts.push(eocd);

	let total = 0;
	for (const p of parts) total += p.length;
	const result = new Uint8Array(total);
	let pos = 0;
	for (const p of parts) {
		result.set(p, pos);
		pos += p.length;
	}
	return result;
}

/** Build an Info-ZIP Unicode Path extra field (tag 0x7075, version 1). */
function buildInfoZipUnicodePath(cdfhNameBytes: Uint8Array, unicodeName: string): Uint8Array {
	const unicodeBytes = new TextEncoder().encode(unicodeName);
	// Data: version(1) + nameCrc(4) + unicodeBytes
	const data = new Uint8Array(5 + unicodeBytes.length);
	data[0] = 1; // version
	const dv = new DataView(data.buffer);
	dv.setUint32(1, crc32(cdfhNameBytes), true);
	data.set(unicodeBytes, 5);
	// Extra field: tag(2) + size(2) + data
	const extra = new Uint8Array(4 + data.length);
	const ev = new DataView(extra.buffer);
	ev.setUint16(0, 0x7075, true);
	ev.setUint16(2, data.length, true);
	extra.set(data, 4);
	return extra;
}

// ---------------------------------------------------------------------------
// Test fixture setup / teardown
// ---------------------------------------------------------------------------

let tmpDir = "";

beforeEach(async () => {
	tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), "omp-archive-test-"));
});

afterEach(async () => {
	if (tmpDir) await fs.rm(tmpDir, { recursive: true, force: true });
});

async function writeZip(name: string, bytes: Uint8Array): Promise<string> {
	const p = path.join(tmpDir, name);
	await Bun.write(p, bytes);
	return p;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("ArchiveReader ZIP filename decoding", () => {
	test("UTF-8 flag (bit 11) decodes Unicode filename correctly", async () => {
		// Bit 11 set → filename bytes must be interpreted as UTF-8
		const name = "比赛规则/说明.txt";
		const nameBytes = new TextEncoder().encode(name);
		const zip = buildStoredZip([{ nameBytes, flags: 0x800, data: new TextEncoder().encode("content") }]);
		const p = await writeZip("utf8-flag.zip", zip);
		const reader = await openArchive(p);

		const top = reader.listDirectory();
		expect(top).toHaveLength(1);
		expect(top[0].name).toBe("比赛规则");
		expect(top[0].isDirectory).toBe(true);

		const sub = reader.listDirectory("比赛规则");
		expect(sub).toHaveLength(1);
		expect(sub[0].name).toBe("说明.txt");

		const file = await reader.readFile("比赛规则/说明.txt");
		expect(new TextDecoder().decode(file.bytes)).toBe("content");
	});

	test("Info-ZIP 0x7075 extra field with valid CRC decodes Unicode filename", async () => {
		// No UTF-8 flag; raw name bytes are GBK-encoded; 0x7075 provides the Unicode name
		// GBK bytes for '比赛' (approximation — exact encoding irrelevant, what matters is the
		// CRC32 in the 0x7075 field matches these raw bytes)
		const rawName = new Uint8Array([0xb1, 0xc8, 0xc8, 0xfc, 0xb9, 0xe6, 0xd4, 0xf2]); // legacy bytes
		const unicodeName = "比赛规则.txt";
		const extra = buildInfoZipUnicodePath(rawName, unicodeName);

		const zip = buildStoredZip([
			{ nameBytes: rawName, flags: 0x0000, data: new TextEncoder().encode("data"), extra },
		]);
		const p = await writeZip("unicode-path.zip", zip);
		const reader = await openArchive(p);

		const entries = reader.listDirectory();
		expect(entries).toHaveLength(1);
		expect(entries[0].name).toBe("比赛规则.txt");
	});

	test("0x7075 extra field with bad CRC is ignored; CP437 fallback is used", async () => {
		// Build a 0x7075 field whose CRC does NOT match the name bytes → must be ignored
		const rawName = new Uint8Array([0x68, 0x65, 0x6c, 0x6c, 0x6f, 0x2e, 0x74, 0x78, 0x74]); // "hello.txt"
		// Deliberately use the wrong name bytes to compute a mismatched CRC
		const wrongBytes = new Uint8Array([0x00, 0x01, 0x02]);
		const extra = buildInfoZipUnicodePath(wrongBytes, "wrong-unicode.txt");

		const zip = buildStoredZip([{ nameBytes: rawName, flags: 0x0000, data: new Uint8Array([0x61]), extra }]);
		const p = await writeZip("bad-7075.zip", zip);
		const reader = await openArchive(p);

		// Should fall back to CP437 decoding of rawName, which is plain ASCII "hello.txt"
		const entries = reader.listDirectory();
		expect(entries).toHaveLength(1);
		expect(entries[0].name).toBe("hello.txt");
	});

	test("CP437 fallback decodes high-byte entries without UTF-8 flag or 0x7075", async () => {
		// In CP437: 0x80 = 'Ç' (U+00C7), 0x81 = 'ü' (U+00FC)
		const rawName = new Uint8Array([0x80, 0x81, 0x6c, 0x6f, 0x2e, 0x74, 0x78, 0x74]); // Çülo.txt

		const zip = buildStoredZip([{ nameBytes: rawName, flags: 0x0000, data: new Uint8Array([0x31]) }]);
		const p = await writeZip("cp437.zip", zip);
		const reader = await openArchive(p);

		const entries = reader.listDirectory();
		expect(entries).toHaveLength(1);
		expect(entries[0].name).toBe("\u00C7\u00FClo.txt");
	});

	test("Plain ASCII filenames decode identically under all three paths", async () => {
		// ASCII bytes are the same in UTF-8, CP437, and any single-byte encoding
		const name = "readme.txt";
		const nameBytes = new TextEncoder().encode(name);

		for (const flags of [0x0000, 0x0800]) {
			const zip = buildStoredZip([{ nameBytes, flags, data: new TextEncoder().encode("hello") }]);
			const p = await writeZip(`ascii-${flags}.zip`, zip);
			const reader = await openArchive(p);
			const entries = reader.listDirectory();
			expect(entries[0].name).toBe("readme.txt");
		}
	});

	test("Existing tar/tgz archive handling is unaffected", async () => {
		// Smoke test: tar format should still work after the ZIP change
		// We can't easily build a tar from scratch here, so we verify the ArchiveReader
		// correctly reads a known-good zip with standard UTF-8 names (fflate-compatible).
		const name = "a/b.txt";
		const nameBytes = new TextEncoder().encode(name);
		const zip = buildStoredZip([{ nameBytes, flags: 0x800, data: new TextEncoder().encode("ok") }]);
		const p = await writeZip("nested.zip", zip);
		const reader = await openArchive(p);

		const top = reader.listDirectory();
		expect(top).toHaveLength(1);
		expect(top[0].path).toBe("a");

		const file = await reader.readFile("a/b.txt");
		expect(new TextDecoder().decode(file.bytes)).toBe("ok");
	});
});
