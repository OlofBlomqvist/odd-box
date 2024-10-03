import Button from "@/components/button/button";
import Input from "@/components/input/input";
import {
  Sheet,
  SheetClose,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from "@/components/sheet/sheet";
import SettingsSection from "@/pages/settings/settings-section";

export const ArgsSheet = ({
  show,
  value,
  originalValue,
  onClose,
  valueChanged,
  onAddArg,
  onRemoveArg,
}: {
  valueChanged: (value: string) => void;
  onClose: () => void;
  show: boolean;
  value: string;
  originalValue: string | undefined;
  onAddArg: (arg: string, originalValue: string | undefined) => void;
  onRemoveArg: (arg: string) => void;
}) => {
  return (
    <Sheet open={show} onOpenChange={onClose}>
      <SheetContent className="bg-[#242424] border-l-[#ffffff10] w-full">
        <SheetHeader className="text-left">
          <SheetTitle className="text-white">
            {originalValue !== "" ? "Edit argument" : "New argument"}
          </SheetTitle>
          <SheetDescription>
            {originalValue === ""
              ? "Add a new argument"
              : `Making changes to '${originalValue}'`}
          </SheetDescription>
        </SheetHeader>

        <SettingsSection marginTop="10px" noBottomSeparator>
          <div
            style={{ display: "flex", flexDirection: "column", gap: "10px" }}
          >
            <div>
              <Input
                withSaveButton placeholder="Argument here.."
                originalValue={originalValue}
                onSave={() => {
                  onAddArg(value, originalValue);
                  onClose();
                }}
                type="text"
                value={value}
                onChange={(e) => valueChanged(e.target.value)}
              />
            </div>
          </div>
        </SettingsSection>

        <SheetFooter className="flex flex-row gap-4">
          {originalValue && (
            <Button
              onClick={() => {
                onRemoveArg(originalValue);
                onClose();
              }}
              style={{
                width: "150px",
                whiteSpace: "nowrap",
                display: "flex",
                alignItems: "center",
                gap: "5px",
                justifyContent: "center",
              }}
              dangerButton
            >
              Delete
            </Button>
          )}
          <SheetClose asChild>
            <Button type="submit">Close</Button>
          </SheetClose>
        </SheetFooter>
      </SheetContent>
    </Sheet>
  );
};
