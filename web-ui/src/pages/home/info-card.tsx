import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { cn } from "@/lib/cn";
import { ReactNode } from "react";

type InfoCardProps = {
  title: ReactNode;
  icon: ReactNode;
  leftData: { label: string; value: number };
  rightData?: { label: string; value: number };
};

const InfoCard = ({ title, icon, leftData, rightData }: InfoCardProps) => {
  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2 justify-between">
          {title}
          {icon}
        </CardTitle>
      </CardHeader>
      <CardContent>
        <div className="flex items-center justify-between">
          <div className="text-4xl font-bold">{leftData.value}</div>
          <div
            className={cn("text-4xl font-bold hidden", rightData && "block")}
          >
            {rightData?.value}
          </div>
        </div>
        <div className="text-sm text-muted-foreground flex justify-between">
          <p>Total</p>
          <p className={cn("hidden", rightData && "block")}>Running</p>
        </div>
      </CardContent>
    </Card>
  );
};

export default InfoCard;
