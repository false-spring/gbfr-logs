import { Button } from "@mantine/core";
import { UploadSimple } from "@phosphor-icons/react";
import { getVersion } from "@tauri-apps/api/app";
import { open } from "@tauri-apps/api/shell";
import { useCallback, useState } from "react";
import toast from "react-hot-toast";
import { useTranslation } from "react-i18next";

import { UploadError, uploadLogs } from "@/utils/upload";

export type UploadSelectedButtonProps = {
  ids: number[];
};

export const UploadSelectedButton = ({ ids }: UploadSelectedButtonProps) => {
  const { t } = useTranslation();
  const [uploading, setUploading] = useState(false);

  const upload = useCallback(async () => {
    if (ids.length === 0) return;
    setUploading(true);
    try {
      const outcome = await uploadLogs(ids, await getVersion());
      const accepted = outcome.stored + outcome.duplicates;
      if (accepted === 0) {
        toast.error(t("ui.upload.all-rejected", "The site could not read any of these encounters."));
        return;
      }
      toast.success(
        t("ui.upload.batch-done", "{{stored}} uploaded, {{duplicates}} already there.", {
          stored: outcome.stored,
          duplicates: outcome.duplicates,
        }) +
          (outcome.rejected > 0
            ? " " +
              t("ui.upload.batch-rejected", "{{rejected}} could not be read.", {
                rejected: outcome.rejected,
              })
            : "")
      );
      if (outcome.url) await open(outcome.url);
    } catch (e) {
      if (e instanceof UploadError && e.status === 429) {
        toast.error(t("ui.upload.rate-limited", "Too many uploads — please try again later."));
      } else if (e instanceof UploadError && e.status === 413) {
        toast.error(t("ui.upload.too-large", "This encounter is too large to upload."));
      } else if (e instanceof UploadError && e.status === 0) {
        toast.error(t("ui.upload.offline", "Could not reach the site."));
      } else {
        toast.error(`${t("ui.upload.error", "Upload failed.")} ${e}`);
      }
    } finally {
      setUploading(false);
    }
  }, [ids, t]);

  return (
    <Button size="xs" variant="default" leftSection={<UploadSimple size={14} />} loading={uploading} onClick={upload}>
      {t("ui.logs.upload-selected-btn", "Upload Selected ({{count}})", { count: ids.length })}
    </Button>
  );
};
