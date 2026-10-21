import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { InvokeResponse } from "@/models/response";
import {
  GTaskEventPayload,
  PTaskEventPayload,
  TaskGroupDesc,
  TaskParam,
} from "@/models/task";
import { responseHandler } from "@/utils/response";

export const TASK_PROGRESS_UPDATE_EVENT = "task:progress-update";
export const TASK_GROUP_UPDATE_EVENT = "task:group-update";

// Progressive task events arrive per task and can burst (a large download
// schedules one created event per file). Buffer them and dispatch in one go so
// React batches a single re-render per tick instead of one per event.
const TASK_PROGRESS_BATCH_INTERVAL_MS = 16;
const TASK_PROGRESS_BATCH_MAX_SIZE = 256;

/**
 * Service class for managing tasks.
 */
export class TaskService {
  /**
   * SCHEDULE a group of progressive tasks.
   * @param taskGroup - The name of the task group.
   * @param params - The parameters for the tasks to be scheduled.
   * @param withTimestamp - Whether to append a timestamp to the task group name for uniqueness.
   *                      Defaults to true.
   * @returns {Promise<InvokeResponse<TaskGroupDesc>>}
   */
  @responseHandler("task")
  static async scheduleProgressiveTaskGroup(
    taskGroup: string,
    params: TaskParam[],
    withTimestamp: boolean = true
  ): Promise<InvokeResponse<TaskGroupDesc>> {
    return await invoke("schedule_progressive_task_group", {
      taskGroup,
      params,
      withTimestamp,
    });
  }

  /**
   * CANCEL a task.
   * @param taskId - The ID of the progressive task to be cancelled.
   * @returns {Promise<InvokeResponse<null>>}
   */
  @responseHandler("task")
  static async cancelProgressiveTask(
    taskId: number
  ): Promise<InvokeResponse<null>> {
    return await invoke("cancel_progressive_task", { taskId });
  }

  /**
   * RESUME a task.
   * @param taskId - The ID of the progressive task to be resumed.
   * @returns {Promise<InvokeResponse<null>>}
   */
  @responseHandler("task")
  static async resumeProgressiveTask(
    taskId: number
  ): Promise<InvokeResponse<null>> {
    return await invoke("resume_progressive_task", { taskId });
  }

  /**
   * STOP a task.
   * @param taskId - The ID of the progressive task to be stopped.
   * @returns {Promise<InvokeResponse<null>>}
   */
  @responseHandler("task")
  static async stopProgressiveTask(
    taskId: number
  ): Promise<InvokeResponse<null>> {
    return await invoke("stop_progressive_task", { taskId });
  }

  /**
   * CANCEL a task group.
   * @param taskGroup - The name of the task group to be cancelled.
   * @returns {Promise<InvokeResponse<null>>}
   *
   */
  @responseHandler("task")
  static async cancelProgressiveTaskGroup(
    taskGroup: string
  ): Promise<InvokeResponse<null>> {
    return await invoke("cancel_progressive_task_group", { taskGroup });
  }

  /**
   * STOP a task group.
   * @param taskGroup - The name of the task group to be stopped.
   * @returns {Promise<InvokeResponse<null>>}
   *
   */
  @responseHandler("task")
  static async stopProgressiveTaskGroup(
    taskGroup: string
  ): Promise<InvokeResponse<null>> {
    return await invoke("stop_progressive_task_group", { taskGroup });
  }

  /**
   * RESUME a task group.
   * @param taskGroup - The name of the task group to be resumed.
   * @returns {Promise<InvokeResponse<null>>}
   */
  @responseHandler("task")
  static async resumeProgressiveTaskGroup(
    taskGroup: string
  ): Promise<InvokeResponse<null>> {
    return await invoke("resume_progressive_task_group", { taskGroup });
  }

  /**
   * DELETE a task group record only when it is non-active (i.e., not Started/Stopped).
   * @param taskGroup - The name of the task group to be deleted.
   * @returns {Promise<InvokeResponse<null>>}
   *
   */
  @responseHandler("task")
  static async deleteProgressiveTaskGroup(
    taskGroup: string
  ): Promise<InvokeResponse<null>> {
    return await invoke("delete_progressive_task_group", { taskGroup });
  }

  /**
   * RETRIEVE the list of progressive tasks.
   * @returns {Promise<InvokeResponse<TaskGroupDesc[]>>}
   */
  @responseHandler("task")
  static async retrieveProgressiveTaskList(): Promise<
    InvokeResponse<TaskGroupDesc[]>
  > {
    return await invoke("retrieve_progressive_task_list");
  }

  /**
   * Listen for updates to progressive tasks.
   * @param callback - The callback to be invoked when a task update occurs.
   */
  static onProgressiveTaskUpdate(
    callback: (payload: PTaskEventPayload) => void
  ): () => void {
    let pending: PTaskEventPayload[] = [];
    let timer: ReturnType<typeof setTimeout> | null = null;

    const flush = () => {
      if (timer !== null) {
        clearTimeout(timer);
        timer = null;
      }
      if (pending.length === 0) return;
      const batch = pending;
      pending = [];
      for (const payload of batch) {
        try {
          callback(payload);
        } catch (error) {
          logger.error("Failed to handle task progress event:", error);
        }
      }
    };

    const unlisten = getCurrentWebview().listen<PTaskEventPayload>(
      TASK_PROGRESS_UPDATE_EVENT,
      (event) => {
        pending.push(event.payload);
        // Bulk scheduling (e.g. thousands of download tasks created at once)
        // must not grow the buffer without bound or delay the UI too long.
        if (pending.length >= TASK_PROGRESS_BATCH_MAX_SIZE) {
          flush();
          return;
        }
        if (timer === null) {
          timer = setTimeout(flush, TASK_PROGRESS_BATCH_INTERVAL_MS);
        }
      }
    );

    return () => {
      unlisten.then((f) => f());
      if (timer !== null) {
        clearTimeout(timer);
        timer = null;
      }
      pending = [];
    };
  }

  /**
   * Listen for task group updates.
   * @param callback - The callback to be invoked when a task group update occurs.
   */

  static onTaskGroupUpdate(
    callback: (payload: GTaskEventPayload) => void
  ): () => void {
    const unlisten = getCurrentWebview().listen<GTaskEventPayload>(
      TASK_GROUP_UPDATE_EVENT,
      (event) => {
        callback(event.payload);
      }
    );
    return () => {
      unlisten.then((f) => f());
    };
  }
}
