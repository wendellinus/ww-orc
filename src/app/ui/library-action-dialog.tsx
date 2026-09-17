import { useId, useRef, useState } from "react";
import { Button } from "@/shared/ui/button";
import { Input } from "@/shared/ui/input";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/shared/ui/dialog";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/shared/ui/alert-dialog";

export type LibraryAction = {
  title: string;
  description: string;
  initialName?: string;
  submitLabel: string;
  onSubmit: (name: string) => Promise<void>;
};

export function LibraryActionDialog({
  action,
  onClose,
}: {
  action: LibraryAction;
  onClose: () => void;
}) {
  const [name, setName] = useState(action.initialName ?? "");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const submitting = useRef(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const inputId = useId();
  const errorId = useId();
  const editingName = action.initialName !== undefined;

  async function submit() {
    if (submitting.current || (editingName && !name.trim())) return;
    submitting.current = true;
    setPending(true);
    setError(null);
    try {
      await action.onSubmit(name.trim());
      onClose();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      submitting.current = false;
      setPending(false);
    }
  }

  function changeOpen(open: boolean) {
    if (!open && !submitting.current) onClose();
  }

  const errorMessage = error ? (
    <p
      id={errorId}
      role="alert"
      className="text-sm wrap-break-word text-destructive"
    >
      {error}
    </p>
  ) : null;

  if (!editingName) {
    return (
      <AlertDialog open onOpenChange={changeOpen}>
        <AlertDialogContent initialFocus={cancelRef}>
          <AlertDialogHeader>
            <AlertDialogTitle>{action.title}</AlertDialogTitle>
            <AlertDialogDescription className="wrap-break-word">
              {action.description}
            </AlertDialogDescription>
          </AlertDialogHeader>
          {errorMessage}
          <AlertDialogFooter>
            <AlertDialogCancel ref={cancelRef} disabled={pending}>
              取消
            </AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              disabled={pending}
              onClick={() => void submit()}
            >
              {pending ? "正在删除…" : action.submitLabel}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    );
  }

  return (
    <Dialog open onOpenChange={changeOpen}>
      <DialogContent showCloseButton={false} initialFocus={inputRef}>
        <DialogHeader>
          <DialogTitle>{action.title}</DialogTitle>
          <DialogDescription>{action.description}</DialogDescription>
        </DialogHeader>
        <form
          className="space-y-6"
          onSubmit={(event) => {
            event.preventDefault();
            void submit();
          }}
        >
          <div className="space-y-2">
            <Input
              ref={inputRef}
              id={inputId}
              value={name}
              onChange={(event) => setName(event.target.value)}
              onFocus={(event) => event.currentTarget.select()}
              disabled={pending}
              required
              autoComplete="off"
              placeholder="输入工作空间名称"
              aria-describedby={error ? errorId : undefined}
            />
            {errorMessage}
          </div>
          <DialogFooter>
            <DialogClose
              render={
                <Button type="button" variant="outline" disabled={pending} />
              }
            >
              取消
            </DialogClose>
            <Button type="submit" disabled={pending || !name.trim()}>
              {pending ? "正在保存…" : action.submitLabel}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
