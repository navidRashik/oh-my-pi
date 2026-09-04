/** Maximum serialized characters exposed for one structured `display()` value. */
const MAX_DISPLAY_TEXT_LENGTH = 8000;

/** Serialize one structured `display()` value for model or expanded TUI output. */
export function formatDisplayJsonForText(value: unknown): string {
	let text: string;
	try {
		text = JSON.stringify(value, null, 2) ?? String(value);
	} catch {
		text = String(value);
	}
	if (text.length > MAX_DISPLAY_TEXT_LENGTH) {
		text = `${text.slice(0, MAX_DISPLAY_TEXT_LENGTH)}\n[…${text.length - MAX_DISPLAY_TEXT_LENGTH}ch elided…]`;
	}
	return text;
}
