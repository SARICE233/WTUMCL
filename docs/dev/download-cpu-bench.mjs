// Microbenchmark for the frontend progressive-task aggregation cost.
// Replicates the reducer in src/contexts/task.tsx (updateGroupInfo + the
// Created / InProgress branches of the task:progress-update listener) so the
// per-event main-thread cost can be measured without a Tauri webview.
//
// Usage: node docs/dev/download-cpu-bench.mjs [taskCount]

const TASK_COUNT = Number(process.argv[2] || 3000);

const TaskDescStatusEnums = {
  Stopped: "Stopped",
  Cancelled: "Cancelled",
  Completed: "Completed",
  InProgress: "InProgress",
  Failed: "Failed",
  Waiting: "Waiting",
};

function updateGroupInfo(group) {
  group.finishedCount = group.taskDescs.filter(
    (t) => t.status === TaskDescStatusEnums.Completed
  ).length;

  let knownTotalArr = group.taskDescs.filter((t) => t.total && t.total > 0);
  let knownTotal = knownTotalArr.reduce((acc, t) => acc + t.total, 0);
  let knownCurrent = knownTotalArr.reduce((acc, t) => acc + (t.current || 0), 0);
  let estimatedTotal;
  if (knownTotalArr.length > 0) {
    estimatedTotal =
      knownTotal +
      (group.taskDescs.length - knownTotalArr.length) *
        (knownTotal / knownTotalArr.length);
  } else {
    estimatedTotal = knownTotal;
  }
  group.progress = estimatedTotal ? (knownCurrent * 100) / estimatedTotal : 0;
  group.estimatedTime = undefined;
  group.taskDescs.forEach((t) => {
    if (t.status === TaskDescStatusEnums.InProgress && t.estimatedTime) {
      if (!group.estimatedTime || group.estimatedTime.secs < t.estimatedTime.secs) {
        group.estimatedTime = t.estimatedTime;
      }
    }
    t.progress = t.total ? (t.current * 100) / t.total : 0;
  });
  group.taskDescs.sort((a, b) => {
    let level = (desc) => {
      switch (desc.status) {
        case TaskDescStatusEnums.Failed: return 0;
        case TaskDescStatusEnums.InProgress: return 1;
        case TaskDescStatusEnums.Waiting: return 2;
        case TaskDescStatusEnums.Completed: return 4;
        default: return 3;
      }
    };
    return level(a) - level(b);
  });
}

function taskDesc(id) {
  return {
    taskId: id,
    taskGroup: "game-client@1",
    payload: { taskType: "download", src: "https://example.invalid/x.jar", dest: "/tmp/x.jar", filename: "x.jar", sha1: "" },
    current: 0,
    total: 1_000_000,
    status: TaskDescStatusEnums.Waiting,
  };
}

let tasks = [];

function handleCreated(id) {
  tasks = (() => {
    const prevTasks = tasks;
    const group = prevTasks.find((t) => t.taskGroup === "game-client@1");
    if (group) {
      group.taskDescs.unshift(taskDesc(id));
      updateGroupInfo(group);
    } else {
      const newGroup = {
        status: "Started",
        taskGroup: "game-client@1",
        taskDescs: [taskDesc(id)],
      };
      updateGroupInfo(newGroup);
      return [newGroup, ...prevTasks];
    }
    return [...prevTasks];
  })();
}

function handleInProgress(id) {
  tasks = (() => {
    const prevTasks = tasks;
    const group = prevTasks.find((t) => t.taskGroup === "game-client@1");
    if (!group) return prevTasks;
    group.taskDescs = group.taskDescs.map((t) => {
      if (t.taskId === id) {
        t.current = Math.min(t.total, t.current + 65536);
        t.status = TaskDescStatusEnums.InProgress;
      }
      return t;
    });
    updateGroupInfo(group);
    return [...prevTasks];
  })();
}

function hr(fn) {
  const start = process.hrtime.bigint();
  fn();
  return Number(process.hrtime.bigint() - start) / 1e6;
}

const createdMs = hr(() => {
  for (let i = 0; i < TASK_COUNT; i++) handleCreated(i);
});

const progressEvents = 200;
const progressMs = hr(() => {
  for (let i = 0; i < progressEvents; i++) handleInProgress(i % TASK_COUNT);
});

const totalOps = TASK_COUNT + progressEvents;
console.log(
  JSON.stringify(
    {
      taskCount: TASK_COUNT,
      createdEvents: TASK_COUNT,
      createdTotalMs: +createdMs.toFixed(1),
      createdPerEventMs: +(createdMs / TASK_COUNT).toFixed(3),
      progressEvents,
      progressTotalMs: +progressMs.toFixed(1),
      progressPerEventMs: +(progressMs / progressEvents).toFixed(3),
      totalMs: +(createdMs + progressMs).toFixed(1),
      perEventMs: +((createdMs + progressMs) / totalOps).toFixed(3),
    },
    null,
    2
  )
);
