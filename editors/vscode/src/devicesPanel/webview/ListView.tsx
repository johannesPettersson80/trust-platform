// The list: the same items as the canvas in one table (specification 25 §5.14). One row per
// computer, runtime, connection and device the view gives, with each row's cells taken from the
// item's `list.cells` by column id and drawn under the columns of `ListView.columns`, in order. A
// hidden item has no row and a dimmed item's row is dimmed, exactly as the view gives them; the list
// never decides which items match a search. Clicking a heading sorts the rows by that column's
// displayed text and a second click reverses it, which is view state kept here and changes no data;
// selecting a row opens that item's inspector, the same one the canvas opens.

import React from "react";

import {
  listCellStyle,
  listHeadingButtonStyle,
  listHeadingStyle,
  listRowStyle,
  listStyle,
  listTableStyle,
} from "./styles";
import type { Column, Computer, Device, ListCells } from "./types";

/** One row of the list: an item of the canvas and its cells. */
interface ListRow {
  /** The item's identity. */
  id: string;
  /** The item's kind, as the view gives it. */
  kind: string;
  /** The text of each column, by column key. */
  cells: Record<string, string>;
  /** Whether the search dims the item. */
  dimmed: boolean;
}

/** The text of a row's cell for a column, or empty when the item fills none. */
function cellText(row: ListRow, column: string): string {
  return row.cells[column] ?? "";
}

/** The rows sorted by one column's displayed text, ascending or reversed. */
function sortRows(rows: ListRow[], column: string, ascending: boolean): ListRow[] {
  return [...rows].sort((left, right) => {
    const first = cellText(left, column);
    const second = cellText(right, column);
    const order = first < second ? -1 : first > second ? 1 : 0;
    return ascending ? order : -order;
  });
}

/**
 * Every item of the canvas with its list cells: each computer, its runtimes, their connections and
 * the devices, in the canvas's own order. A hidden item is left out and a dimmed one's row is dimmed,
 * exactly as the view gives them (specification 25 §5.14).
 */
function listRows(computers: Computer[], devices: Device[]): ListRow[] {
  const rows: ListRow[] = [];
  const add = (
    id: string,
    list: ListCells | undefined,
    dimmed: boolean | undefined,
    hidden: boolean | undefined,
  ) => {
    if (list === undefined || hidden === true) {
      return;
    }
    rows.push({ id, kind: list.kind, cells: list.cells, dimmed: dimmed === true });
  };
  for (const computer of computers) {
    add(computer.id, computer.list, computer.dimmed, computer.hidden);
    for (const runtime of computer.runtimes) {
      add(runtime.id, runtime.list, runtime.dimmed, runtime.hidden);
      for (const connection of runtime.connections ?? []) {
        add(connection.id, connection.list, connection.dimmed, connection.hidden);
      }
    }
  }
  for (const device of devices) {
    add(device.id, device.list, device.dimmed, device.hidden);
  }
  return rows;
}

/** Draws the list, sorts it by a clicked heading and opens the inspector of a chosen row's item. */
export function ListView({
  columns,
  computers,
  devices = [],
  onSelectRow,
}: {
  /** The columns, in their order. */
  columns: Column[];
  /** The computers and chips, each with its runtimes and their connections. */
  computers: Computer[];
  /** The devices at the other end of the connections. */
  devices?: Device[];
  /** Called with the identity of the item of the chosen row. */
  onSelectRow: (id: string) => void;
}) {
  const [sort, setSort] = React.useState<{ column: string; ascending: boolean } | undefined>(
    undefined,
  );
  const onHeading = React.useCallback((column: string) => {
    setSort((current) =>
      current !== undefined && current.column === column
        ? { column, ascending: !current.ascending }
        : { column, ascending: true },
    );
  }, []);

  const rows = listRows(computers, devices);
  const ordered =
    sort === undefined ? rows : sortRows(rows, sort.column, sort.ascending);

  return (
    <div data-role="list" style={listStyle}>
      <table style={listTableStyle}>
        <thead>
          <tr>
            {columns.map((column) => (
              <th key={column.id} data-role="list-heading" style={listHeadingStyle}>
                <button
                  type="button"
                  data-column={column.id}
                  onClick={() => onHeading(column.id)}
                  style={listHeadingButtonStyle}
                >
                  {column.label}
                </button>
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {ordered.map((row) => (
            <tr
              key={row.id}
              data-item-id={row.id}
              data-role="list-row"
              data-kind={row.kind}
              data-dimmed={row.dimmed ? "true" : "false"}
              onClick={() => onSelectRow(row.id)}
              style={listRowStyle(row.dimmed)}
            >
              {columns.map((column) => (
                <td key={column.id} data-column={column.id} style={listCellStyle}>
                  {cellText(row, column.id)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
