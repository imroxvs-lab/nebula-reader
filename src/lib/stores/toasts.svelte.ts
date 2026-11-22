// Notificaciones efímeras.

export type ToastKind = "info" | "success" | "error" | "progress";

export interface Toast {
  id: number;
  kind: ToastKind;
  message: string;
  detail?: string;
}

const DURATION_MS: Record<ToastKind, number | null> = {
  info: 4000,
  success: 5000,
  error: 8000,
  progress: null,
};

class ToastStore {
  items = $state<Toast[]>([]);
  #nextId = 1;

  show(kind: ToastKind, message: string, detail?: string): number {
    const id = this.#nextId++;
    this.items.push({ id, kind, message, detail });
    const duration = DURATION_MS[kind];
    if (duration !== null) setTimeout(() => this.dismiss(id), duration);
    return id;
  }

  /** Sustituye un toast (p. ej. el de progreso) por su resultado. */
  replace(id: number, kind: ToastKind, message: string, detail?: string): void {
    this.dismiss(id);
    this.show(kind, message, detail);
  }

  dismiss(id: number): void {
    this.items = this.items.filter((toast) => toast.id !== id);
  }
}

export const toasts = new ToastStore();
