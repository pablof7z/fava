---
title: "Never bend the E2E app to accommodate bad Fava DX"
summary: "Fix Fava DX; do not work around it in an E2E app."
priority: "MUST"
questions: ["Is there a better way that Fava could provide so that apps would...?", "Does this feature artificially reduce what should be possible by the feature being demonstrated? e.g. e2e fava-accounts prohibiting having more than 5 logged-in accounts.", "Is there unnecessary boilerplate? e.g. e2e must remember which account is currently logged-in just to pass it in every fava.use_account(current-user) call"]
---

# Never bend the E2E app to accommodate bad Fava DX

The whole point of E2E apps is to prove Fava can do things in a satisfying DX and that things just work. If the E2E app is doing uncomfortable work, fix Fava rather than the E2E app.

## Questions

- Is there a better way that Fava could provide so that apps would...?

- Does this feature artificially reduce what should be possible by the feature being demonstrated? e.g. e2e fava-accounts prohibiting having more than 5 logged-in accounts.

- Is there unnecessary boilerplate? e.g. e2e must remember which account is currently logged-in just to pass it in every fava.use_account(current-user) call
