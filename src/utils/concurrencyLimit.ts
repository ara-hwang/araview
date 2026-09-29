// 웹툰 연속 뷰처럼 화면 단위로 동시 invoke가 폭증하는 경로의 동시 실행 상한.
// 상한 초과분은 FIFO로 대기하며, 대기 중 스크롤 아웃된 작업은 호출자의
// task 안에서 다시 취소 여부를 확인해야 한다(여기서는 실행만 미룬다).
const MAX_CONCURRENT_LOADS = 4

let running = 0
const queue: Array<() => void> = []

export function runLimitedImageLoad<T>(task: () => Promise<T>): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const run = () => {
      running += 1
      Promise.resolve()
        .then(task)
        .then(resolve, reject)
        .finally(() => {
          running -= 1
          queue.shift()?.()
        })
    }
    if (running < MAX_CONCURRENT_LOADS) {
      run()
    } else {
      queue.push(run)
    }
  })
}
