export type Draft = { text: string; color: string };
// Serialize writes and use the revision returned by the last successful save.
export class NoteAutosave {
  private pending: Draft | null = null;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private tail: Promise<void> = Promise.resolve();
  private revision: number;
  private save: (
    draft: Draft,
    revision: number,
  ) => Promise<{ revision: number }>;
  private status: (value: string) => void;
  constructor(
    revision: number,
    save: (draft: Draft, revision: number) => Promise<{ revision: number }>,
    status: (value: string) => void,
  ) {
    this.revision = revision;
    this.save = save;
    this.status = status;
  }
  schedule(draft: Draft) {
    this.pending = draft;
    this.status("未保存");
    clearTimeout(this.timer);
    this.timer = setTimeout(() => {
      void this.flush().catch(() => {});
    }, 450);
  }
  flush(): Promise<void> {
    clearTimeout(this.timer);
    const operation = this.tail.then(async () => {
      while (this.pending) {
        const draft = this.pending;
        this.pending = null;
        this.status("保存中…");
        try {
          const saved = await this.save(draft, this.revision);
          this.revision = saved.revision;
        } catch (error) {
          this.pending ??= draft;
          this.status(`保存失败：${String(error)}`);
          throw error;
        }
      }
      this.status("已保存");
    });
    this.tail = operation.catch(() => {});
    return operation;
  }
  dispose() {
    clearTimeout(this.timer);
  }
}
