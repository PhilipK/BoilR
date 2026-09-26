import type { PlatformSettingsPayload } from "../types";

export type BooleanField = {
  key: string;
  label: string;
  kind: "boolean";
  value: boolean;
  originalValue: boolean;
};

export type StringField = {
  key: string;
  label: string;
  kind: "string";
  value: string;
  originalValue: string;
};

export type OptionalStringField = {
  key: string;
  label: string;
  kind: "optional-string";
  value: string;
  originalValue: string | null;
};

export type StringListField = {
  key: string;
  label: string;
  kind: "string-list";
  value: string[];
  originalValue: string[];
};

export type PlatformSettingsField =
  | BooleanField
  | StringField
  | OptionalStringField
  | StringListField;

export type PlatformSettingsGroup = {
  codeName: string;
  name: string;
  fields: PlatformSettingsField[];
};

export type PlatformFieldUpdate =
  | { key: string; kind: "boolean"; value: boolean }
  | { key: string; kind: "string"; value: string }
  | { key: string; kind: "optional-string"; value: string }
  | { key: string; kind: "string-list"; value: string[] };

export const humaniseKey = (raw: string): string =>
  raw
    .replace(/_/g, " ")
    .replace(/\b\w/g, (char) => char.toUpperCase())
    .trim();

const normaliseStringList = (list: string[]): string[] =>
  list.map((entry) => entry.trim()).filter((entry) => entry.length > 0);

export const buildPlatformSettingsGroup = (
  payload: PlatformSettingsPayload
): PlatformSettingsGroup => {
  const entries = Object.entries(payload.settings ?? {}).sort((a, b) => {
    if (a[0] === "enabled") {
      return -1;
    }
    if (b[0] === "enabled") {
      return 1;
    }
    return a[0].localeCompare(b[0]);
  });

  const fields: PlatformSettingsField[] = [];

  for (const [key, raw] of entries) {
    const label = humaniseKey(key);
    if (typeof raw === "boolean") {
      fields.push({ key, label, kind: "boolean", value: raw, originalValue: raw });
      continue;
    }

    if (typeof raw === "string") {
      fields.push({ key, label, kind: "string", value: raw, originalValue: raw });
      continue;
    }

    if (raw === null) {
      fields.push({ key, label, kind: "optional-string", value: "", originalValue: null });
      continue;
    }

    if (Array.isArray(raw) && raw.every((value) => typeof value === "string")) {
      const list = raw as string[];
      fields.push({
        key,
        label,
        kind: "string-list",
        value: [...list],
        originalValue: [...list],
      });
    }
  }

  return {
    codeName: payload.code_name,
    name: payload.name,
    fields,
  };
};

export const mergeOptionalFieldKinds = (
  next: PlatformSettingsGroup,
  previous: PlatformSettingsGroup
): PlatformSettingsGroup => {
  const optionalKeys = new Set(
    previous.fields
      .filter((field) => field.kind === "optional-string")
      .map((field) => field.key)
  );

  if (!optionalKeys.size) {
    return next;
  }

  const fields = next.fields.map((field) => {
    if (field.kind === "string" && optionalKeys.has(field.key)) {
      return {
        key: field.key,
        label: field.label,
        kind: "optional-string" as const,
        value: field.value,
        originalValue: field.value,
      } satisfies OptionalStringField;
    }
    return field;
  });

  return { ...next, fields };
};

const fieldHasChanges = (field: PlatformSettingsField): boolean => {
  switch (field.kind) {
    case "boolean":
      return field.value !== field.originalValue;
    case "string":
      return field.value !== field.originalValue;
    case "optional-string":
      return field.value !== (field.originalValue ?? "");
    case "string-list":
      if (field.value.length !== field.originalValue.length) {
        return true;
      }
      return field.value.some((entry, index) => entry !== field.originalValue[index]);
    default:
      return false;
  }
};

export const groupHasChanges = (group: PlatformSettingsGroup): boolean =>
  group.fields.some((field) => fieldHasChanges(field));

export const resetGroup = (group: PlatformSettingsGroup): PlatformSettingsGroup => {
  const fields = group.fields.map((field) => {
    switch (field.kind) {
      case "boolean":
        return { ...field, value: field.originalValue };
      case "string":
        return { ...field, value: field.originalValue };
      case "optional-string":
        return { ...field, value: field.originalValue ?? "" };
      case "string-list":
        return { ...field, value: [...field.originalValue] };
      default:
        return field;
    }
  });

  return { ...group, fields };
};

export const buildPlatformSettingsPayload = (
  group: PlatformSettingsGroup
): Record<string, unknown> => {
  const payload: Record<string, unknown> = {};
  group.fields.forEach((field) => {
    switch (field.kind) {
      case "boolean":
        payload[field.key] = field.value;
        break;
      case "string":
        payload[field.key] = field.value;
        break;
      case "optional-string":
        payload[field.key] = field.value.trim().length === 0 ? null : field.value;
        break;
      case "string-list":
        payload[field.key] = normaliseStringList(field.value);
        break;
      default:
        break;
    }
  });
  return payload;
};
