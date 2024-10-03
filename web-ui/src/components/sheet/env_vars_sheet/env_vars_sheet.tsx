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
import { KvP } from "@/generated-api";
import SettingsSection from "@/pages/settings/settings-section";

export const EnvVarsSheet = ({
  show,
  value,
  originalValue,
  name,
  onClose,
  valueChanged,
  onRemoveKey,
  onNewKey,
}: {
  name: string;
  valueChanged: (value: string) => void;
  onClose: () => void;
  show: boolean;
  value: string;
  originalValue: string | undefined;
  onRemoveKey?: (keyName: string) => void;
  onNewKey?: (newKey: KvP, originalName: string | undefined) => void;
}) => {
  return (
    <Sheet open={show} onOpenChange={onClose}>
      <SheetContent className="bg-[#242424] border-l-[#ffffff10] w-full">
        <SheetHeader className="text-left">
          <SheetTitle className="text-white">
            {name !== ""
              ? "Edit environment variable"
              : "New environment variable"}
          </SheetTitle>
          <SheetDescription>
            {name !== ""
              ? `Making changes to '${name}'`
              : "Adding a new environment variable"}
          </SheetDescription>
        </SheetHeader>

        <SettingsSection marginTop="10px" noBottomSeparator>
          <div
            style={{ display: "flex", flexDirection: "column", gap: "10px" }}
          >
            {name === "" && (
              <Input
                placeholder="Environment variable name"
                withSaveButton
                originalValue={""}
                onSave={() => {
                  onNewKey?.(
                    {
                      key: value,
                      value: "",
                    },
                    name
                  );
                }}
                type="text"
                value={value}
                onChange={(e) => valueChanged(e.target.value)}
              />
            )}
            {name !== "" && (
              <Input
                placeholder="Environment variable value"
                withSaveButton
                originalValue={originalValue}
                onSave={() => {
                  onNewKey?.(
                    {
                      key: name,
                      value: value,
                    },
                    name
                  );
                  onClose();
                }}
                type="text"
                value={value}
                onChange={(e) => valueChanged(e.target.value)}
              />
            )}
          </div>
        </SettingsSection>
        <SheetFooter className="flex flex-row gap-4">
          {name && (
            <Button
              onClick={() => {
                onRemoveKey?.(name!);
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
            <Button type="submit">Cancel</Button>
          </SheetClose>
        </SheetFooter>
      </SheetContent>
    </Sheet>
  );
};
