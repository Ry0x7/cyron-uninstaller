import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SearchSelectionBar } from "./SearchSelectionBar";

const noop = () => {};

describe("SearchSelectionBar", () => {
  it("renders nothing without a selection", () => {
    expect(
      renderToStaticMarkup(
        <SearchSelectionBar
          count={0}
          bytes={0}
          busy={null}
          onCopy={noop}
          onExportCsv={noop}
          onExportTxt={noop}
          onClear={noop}
        />,
      ),
    ).toBe("");
  });

  it("names every action and shows the selected count and size", () => {
    const html = renderToStaticMarkup(
      <SearchSelectionBar
        count={3}
        bytes={2048}
        busy={null}
        onCopy={noop}
        onExportCsv={noop}
        onExportTxt={noop}
        onClear={noop}
      />,
    );
    expect(html).toContain("3</strong> selected");
    expect(html).toContain("2 KB");
    expect(html).toContain("Copy paths");
    expect(html).toContain('aria-label="Export selected rows as CSV"');
    expect(html).toContain('aria-label="Export selected rows as TXT"');
    expect(html).toContain("Clear selection");
    expect(html).not.toContain("disabled");
  });

  it("disables actions while an export is running", () => {
    const html = renderToStaticMarkup(
      <SearchSelectionBar
        count={1}
        bytes={10}
        busy="csv"
        onCopy={noop}
        onExportCsv={noop}
        onExportTxt={noop}
        onClear={noop}
      />,
    );
    expect(html).toContain("disabled");
    expect(html).toContain("1</strong> selected");
  });
});
