import { describe, expect, it } from "vitest";
import { translate } from "../index";

describe("i18n message helpers", () => {
  it("handles plural messages", () => {
    expect(translate("en", "profiles.shareCopiedMany", { count: 1 })).toBe("Copied 1 link");
    expect(translate("en", "profiles.shareCopiedMany", { count: 3 })).toBe("Copied 3 links");
  });

  it("handles composed plural messages", () => {
    expect(translate("en", "profiles.subtitle", { servers: 1, groups: 2 })).toBe(
      "1 server · 2 groups",
    );
  });

  it("handles pluralized store notifications", () => {
    expect(translate("en", "store.profile.imported", { count: 1 })).toBe("Imported 1 profile");
  });
});
