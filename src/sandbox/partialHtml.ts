// Prepares streaming HTML for the preview frame.

/** Complete <style> blocks in the raw HTML, for injection into <head>. */
export function extractCompleteStyles(html: string): string {
  const matches = html.match(/<style\b[^>]*>[\s\S]*?<\/style>/gi);
  return matches ? matches.join("") : "";
}

/**
 * Makes partial HTML safe to preview. Order matters:
 * 1. drop an unfinished tag at the end;
 * 2. drop complete <style>, <script>, and <head> blocks;
 * 3. drop unfinished ones;
 * 4. drop an unfinished entity at the end;
 * 5. keep only the body content when a <body> tag is present.
 */
export function processPartialHtml(html: string): string {
  let result = html;
  result = result.replace(/<[^>]*$/, "");
  result = result.replace(/<(style|script|head)\b[^>]*>[\s\S]*?<\/\1>/gi, "");
  result = result.replace(/<(style|script|head)\b[^>]*>[\s\S]*$/gi, "");
  result = result.replace(/&[a-zA-Z0-9#]*$/, "");
  const body = result.match(/<body[^>]*>([\s\S]*)/i);
  if (body) {
    result = body[1].replace(/<\/body>[\s\S]*/i, "");
  }
  return result;
}
