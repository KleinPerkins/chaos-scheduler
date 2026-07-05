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
  ]),
  {
    files: ["e2e/**/*.{ts,tsx}", "playwright.config.ts", "vitest.config.ts"],
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
    rules: {
      ...jsxA11yRecommended.rules,
      "jsx-a11y/anchor-is-valid": "off",
    },
  },
]);
