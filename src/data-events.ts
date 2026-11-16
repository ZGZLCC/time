/**
 * 本地数据变更广播。
 *
 * 桌面版各页由 KeepAlive 缓存：记录页只在首次挂载时读一次数据，
 * 而同步、导入、清空都发生在设置页——没有通知的话，回到记录页会一直
 * 停在旧列表上，看起来像「覆盖没生效」。网页版有同样的机制，这里保持一致。
 */
type Listener = () => void;

const listeners = new Set<Listener>();

/** 数据被本页之外的操作改动后调用。 */
export function emitLocalDataChanged(): void {
  for (const listener of [...listeners]) listener();
}

/** 订阅变更，返回取消订阅的函数。 */
export function onLocalDataChanged(listener: Listener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
