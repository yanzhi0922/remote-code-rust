export interface MarkdownTableProps {
  headers: string[];
  rows: string[][];
}

export function MarkdownTable({ headers, rows }: MarkdownTableProps) {
  return (
    <div data-testid="markdown-table" className="overflow-x-auto">
      <table className="w-full border-collapse text-sm">
        <thead>
          <tr>
            {headers.map((header, i) => (
              <th
                key={i}
                data-testid={`markdown-table-header-${i}`}
                className="border-b border-slate-200 bg-slate-50 px-3 py-2 text-left font-medium text-slate-700"
              >
                {header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row, ri) => (
            <tr key={ri} data-testid={`markdown-table-row-${ri}`} className="border-b border-slate-100">
              {row.map((cell, ci) => (
                <td key={ci} className="px-3 py-2 text-slate-600">
                  {cell}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
