import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import ReactMarkdown from "react-markdown";
import { expect, it } from "vitest";

it("renders model HTML as inert text and rejects script URLs", () => {
  const html = renderToStaticMarkup(
    createElement(ReactMarkdown, {
      children: "<script>alert(1)</script> [run](javascript:alert(1))",
    }),
  );
  expect(html).not.toContain("<script>");
  expect(html).not.toContain('href="javascript:');
  expect(html).toContain("&lt;script&gt;");
});
