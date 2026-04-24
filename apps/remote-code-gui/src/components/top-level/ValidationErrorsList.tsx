import { AlertCircle } from 'lucide-react';

export interface ValidationError {
  field: string;
  message: string;
}

export interface ValidationErrorsListProps {
  errors: ValidationError[];
}

export function ValidationErrorsList({ errors }: ValidationErrorsListProps) {
  if (errors.length === 0) return null;

  return (
    <div data-testid="validation-errors-list" className="space-y-1">
      {errors.map((error, i) => (
        <div
          key={i}
          data-testid={`validation-error-${i}`}
          className="flex items-start gap-2 rounded bg-red-50 px-3 py-1.5 text-sm text-red-600"
        >
          <AlertCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
          <span>
            <strong>{error.field}</strong>: {error.message}
          </span>
        </div>
      ))}
    </div>
  );
}
