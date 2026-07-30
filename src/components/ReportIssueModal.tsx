import { Button, Group, Modal, Stack, Text, Textarea, TextInput } from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { Question } from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api";
import { useCallback, useState } from "react";
import toast from "react-hot-toast";
import { useTranslation } from "react-i18next";

import { SITE_BASE_URL } from "@/utils/upload";

export type ReportIssueModalProps = {
  id: string | undefined;
};

export const ReportIssueModal = ({ id }: ReportIssueModalProps) => {
  const { t } = useTranslation();
  const [opened, handlers] = useDisclosure(false);
  const [desc, setDesc] = useState("");
  const [contact, setContact] = useState("");
  const [submitting, setSubmitting] = useState(false);

  const submitBugReport = useCallback(async () => {
    if (!id || desc.trim().length === 0) return;
    setSubmitting(true);
    try {
      const log = await invoke("bug_report_payload", { id: Number(id) });
      const res = await fetch(`${SITE_BASE_URL}/api/bugreport`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          log,
          description: desc.trim(),
          contact: contact.trim() || undefined,
        }),
      });
      if (res.ok) {
        toast.success(t("ui.report-issue.success", "Bug report submitted — thank you!"));
        setDesc("");
        setContact("");
        handlers.close();
      } else if (res.status === 429) {
        toast.error(t("ui.report-issue.rate-limited", "Too many reports — please try again later."));
      } else if (res.status === 413) {
        toast.error(t("ui.report-issue.too-large", "This log is too large to submit."));
      } else {
        toast.error(t("ui.report-issue.error", "Failed to submit bug report."));
      }
    } catch (e) {
      toast.error(`${t("ui.report-issue.error", "Failed to submit bug report.")} ${e}`);
    } finally {
      setSubmitting(false);
    }
  }, [id, desc, contact, handlers, t]);

  return (
    <>
      <Button size="xs" variant="default" leftSection={<Question size={14} />} onClick={handlers.open}>
        {t("ui.report-issue.button", "Report Issue")}
      </Button>
      <Modal opened={opened} onClose={handlers.close} title={t("ui.report-issue.title", "Report an Issue")} size="lg">
        <Stack>
          <Textarea
            label={t("ui.report-issue.description", "Issue description")}
            autosize
            minRows={4}
            maxLength={1500}
            value={desc}
            onChange={(event) => setDesc(event.currentTarget.value)}
            required
          />
          <TextInput
            label={t("ui.report-issue.contact", "(Optional) Your name/contact info")}
            maxLength={100}
            value={contact}
            onChange={(event) => setContact(event.currentTarget.value)}
          />
          <Text size="xs" c="dimmed">
            {t("ui.report-issue.disclaimer", "A copy of this log will be attached so the issue can be reproduced.")}
          </Text>
          <Group justify="flex-end">
            <Button variant="default" onClick={handlers.close}>
              {t("ui.cancel", "Cancel")}
            </Button>
            <Button onClick={submitBugReport} loading={submitting} disabled={desc.trim().length === 0}>
              {t("ui.report-issue.submit", "Submit")}
            </Button>
          </Group>
        </Stack>
      </Modal>
    </>
  );
};
