import { Button } from "@mantine/core";
import { UploadSimple } from "@phosphor-icons/react";
import { getVersion } from "@tauri-apps/api/app";
import { open } from "@tauri-apps/api/shell";
import { useCallback, useState } from "react";
import toast from "react-hot-toast";
import { useTranslation } from "react-i18next";

import { UploadError, uploadLogs } from "@/utils/upload";

export type UploadLogButtonProps = {
  id: string | undefined;
};

export const UploadLogButton = ({ id }: UploadLogButtonProps) => {
  const { t } = useTranslation();
  const [uploading, setUploading] = useState(false);

  const upload = useCallback(async () => {
    if (!id) return;
    setUploading(true);
    try {
      const outcome = await uploadLogs([Number(id)], await getVersion());
      if (!outcome.url) {
        toast.error(t("ui.upload.rejected", "The site could not read this encounter."));
        return;
      }
      await open(outcome.url);
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
  }, [id, t]);

  return (
    <Button size="xs" variant="default" leftSection={<UploadSimple size={14} />} loading={uploading} onClick={upload}>
      {t("ui.upload.view-online", "Upload & view log online")}
    </Button>
  );
};
