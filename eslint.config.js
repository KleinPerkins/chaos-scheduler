import js from "@eslint/js";
import globals from "globals";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import jsxA11y from "eslint-plugin-jsx-a11y";
import tseslint from "typescript-eslint";
import { defineConfig, globalIgnores } from "eslint/config";

const jsxA11yRecommended = jsxA11y.flatConfigs.recommended;

export default defineConfig([
  globalIgnores([
    "dist",
    "src-tauri/target",
    "playwright-report",
    "test-results",
    "**/*.figma.tsx",
  ]),
  {
    files: [
      "e2e/**/*.{ts,tsx}",
      "playwright.config.ts",
      "playwright.visual.config.ts",
      "vitest.config.ts",
    ],
    languageOptions: {
      ecmaVersion: 2020,
      globals: globals.node,
    },
  },
  {
    files: ["**/*.{ts,tsx}"],
    extends: [
      js.configs.recommended,
      tseslint.configs.recommended,
      reactHooks.configs.flat.recommended,
      reactRefresh.configs.vite,
    ],
    languageOptions: {
      ecmaVersion: 2020,
      globals: globals.browser,
    },
  },
  {
    files: ["src/**/*.{ts,tsx}"],
    ...jsxA11yRecommended,
    settings: {
      ...jsxA11yRecommended.settings,
      // The `Input` / `Select` / `Textarea` primitives (src/components/Input.tsx,
      // src/components/Select.tsx, src/components/Textarea.tsx) are thin,
      // class-less wrappers that render a native `<input>` / `<select>` /
      // `<textarea>`, so a11y rules (e.g. label-has-associated-control) must
      // treat them as those controls, not as new unknown elements. `EnvSelect`
      // (src/components/EnvSelect.tsx) composes `<Select>` and likewise renders
      // a native `<select>`, so it maps to `select` too.
      "jsx-a11y": {
        components: {
          Input: "input",
          Select: "select",
          EnvSelect: "select",
          Textarea: "textarea",
        },
      },
    },
    rules: {
      ...jsxA11yRecommended.rules,
      "jsx-a11y/anchor-is-valid": "off",
    },
  },
]);
