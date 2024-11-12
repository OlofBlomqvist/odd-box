import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/dialog/dialog";
import { Button } from "../../ui/button";
import { ReactNode } from "react";
import { Loader2 } from "lucide-react";
export function ConfirmationDialog({
  title,
  subtitle,
  onConfirm,
  onClose,
  show,
  noBtnText,
  yesBtnText,
  isDangerAction,
  isSuccessLoading,
  inProgressText
}: {
  inProgressText?:string
  isDangerAction?: boolean;
  noBtnText?: string;
  yesBtnText?: string;
  title: ReactNode;
  subtitle: ReactNode;
  onConfirm: () => Promise<void>;
  onClose: () => void;
  show: boolean;
  isSuccessLoading?:boolean
}) {
  return (
    <Dialog open={show} onOpenChange={onClose}>
      <DialogContent className="sm:max-w-[425px] bg-[var(--modal-bg)] border border-[#242424]">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>{subtitle}</DialogDescription>
        </DialogHeader>
        <DialogFooter className="gap-2">
          <Button onClick={onClose} variant="secondary" type="button">
            {noBtnText ?? "No, cancel"}
          </Button>
          <Button
            disabled={isSuccessLoading}
            onClick={onConfirm}
            variant={isDangerAction ? "destructive" : "default"}
          >
            {isSuccessLoading && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
            {isSuccessLoading && inProgressText ? inProgressText : (yesBtnText ?? "Yes, confirm")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
