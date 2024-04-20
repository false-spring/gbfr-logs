declare module "jsurl" {
  type Nullable<T> = T | null | undefined;
  export function stringify(input: unknown): string;
  export function parse(input?: Nullable<string>): Nullable<unknown>;
  export function tryParse(input?: Nullable<string>, def?: unknown): Nullable<unknown>;
}
