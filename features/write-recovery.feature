Feature: Accepted publication obligations survive process death

  # fava:id=WRITE-RECOVERY-001
  # fava:status=built
  # fava:evidence=canary:crash-after-acceptance
  # fava:evidence=rust:fava-write-store-redb::every_m5_commit_and_effect_boundary_survives_sigkill_exactly
  # fava:falsifier=Return acceptance without committing its receipt; SIGKILL leaves no reattachable obligation and recovery fails.
  @acceptance @real-relay @process-kill
  Scenario: Acceptance resumes with the same receipt after SIGKILL
    Given Fava durably accepted an unsigned event and returned a receipt
    When the process is killed before signing and restarted on the same write store
    Then the same receipt and event identity are queryable without resubmission
    And Fava signs and publishes the recovered obligation
    And the real relay serves the recovered event

  # fava:id=WRITE-RECOVERY-002
  # fava:status=built
  # fava:evidence=rust:fava-write-store-redb::every_m5_commit_and_effect_boundary_survives_sigkill_exactly
  # fava:falsifier=Recover an in-flight attempt as definitely not handed off; retry can duplicate an effect that may already have happened.
  @process-kill
  Scenario: Every durable boundary recovers an exact safe fact
    Given acceptance, signature, attempt authorization, outcome, and cancellation commits
    When SIGKILL occurs at each commit or effect boundary
    Then pre-commit work is absent
    And committed work retains the same stable receipt
    And an interrupted attempt recovers as unknown rather than definitely failed
