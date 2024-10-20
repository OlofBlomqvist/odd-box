import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/dialog/dialog";
import {Button} from "../../ui/button";
export function ConfirmationDialog({
  title,
  subtitle,
  onConfirm,
  onClose,
  show,
  noBtnText,
  yesBtnText,
  isDangerAction,
}: {
  isDangerAction?:boolean
  noBtnText?: string;
  yesBtnText?: string;
  title: string;
  subtitle: string;
  onConfirm: () => void;
  onClose: () => void;
  show: boolean;
}) {
  return (
    <Dialog open={show} onOpenChange={onClose}>
      <DialogContent className="sm:max-w-[425px] bg-[#09090b]">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>{subtitle}</DialogDescription>
        </DialogHeader>
        <DialogFooter className="gap-2">
          <Button onClick={onClose} variant="secondary" type="button">
            {noBtnText ?? "No, cancel"}
          </Button>
          <Button onClick={onConfirm} variant={isDangerAction ? "destructive" : "default"}>
            {yesBtnText ?? "Yes, confirm"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
