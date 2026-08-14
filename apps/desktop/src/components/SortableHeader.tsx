import { useEffect, useRef, useState } from "react";

import { clampColumnWidth, type NodeSort, type NodeSortColumn } from "../appHelpers";

interface SortableHeaderProps {
  /// Absent for a column that carries no order the table can apply.
  column?: NodeSortColumn;
  /// The Chinese source label; the component translates it for display and
  /// keeps the source text in the accessible name so word order never depends
  /// on the language.
  label: string;
  sort: NodeSort | null;
  width: number | undefined;
  t: (text: string) => string;
  onSort: (column: NodeSortColumn) => void;
  onResize: (label: string, width: number) => void;
}

/** A column header a desktop grid would give you: click to order, drag to size. */
export function SortableHeader({
  column,
  label,
  sort,
  width,
  t,
  onSort,
  onResize,
}: SortableHeaderProps) {
  const active =
    column !== undefined && sort?.column === column ? sort.direction : null;
  const header = useRef<HTMLTableCellElement>(null);
  const [drag, setDrag] = useState<{ startX: number; startWidth: number } | null>(
    null,
  );

  useEffect(() => {
    if (drag === null) {
      return undefined;
    }
    const move = (event: MouseEvent) => {
      onResize(label, clampColumnWidth(drag.startWidth + event.clientX - drag.startX));
    };
    const stop = () => setDrag(null);
    // Bound to the window: the pointer leaves the six-pixel grip immediately.
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", stop);
    return () => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", stop);
    };
  }, [drag, label, onResize]);

  return (
    <th
      ref={header}
      aria-sort={
        column === undefined
          ? undefined
          : active === "asc"
            ? "ascending"
            : active === "desc"
              ? "descending"
              : "none"
      }
      style={width === undefined ? undefined : { width: `${width}px` }}
    >
      {column === undefined ? (
        t(label)
      ) : (
        <button
          type="button"
          className="column-sort"
          aria-label={`按${label}排序表头`}
          onClick={() => onSort(column)}
        >
          {t(label)}
          {active === "asc" ? " ▲" : active === "desc" ? " ▼" : ""}
        </button>
      )}
      <span
        className="column-grip"
        role="separator"
        aria-label={`调整${label}列宽`}
        onMouseDown={(event) => {
          event.preventDefault();
          setDrag({
            startX: event.clientX,
            startWidth: width ?? header.current?.offsetWidth ?? 120,
          });
        }}
      />
    </th>
  );
}
