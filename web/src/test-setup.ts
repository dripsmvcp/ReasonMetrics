// Global vitest setup: unmount/clean up whatever @testing-library/react
// rendered after each test. Without this, DOM trees from earlier tests in
// the same file pile up in document.body, and any query that isn't scoped
// to a specific render's container (screen.getByRole, etc.) starts matching
// multiple elements.
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

afterEach(() => {
  cleanup();
});
