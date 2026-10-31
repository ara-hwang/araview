# 벤치 기준선

`cargo bench -p araview-core`를 돌린 뒤 `node scripts/bench.mjs record --label <버전>`으로
저장한 측정값입니다. 파일마다 벤치별 p50·p95·평균(ns)과 표본 수가 들어 있고, 릴리스
전후 비교는 `node scripts/bench.mjs compare <기준선.json>`으로 합니다.

- 이름은 릴리스 버전을 따릅니다(예: `2.7.0.json`). 같은 버전을 다시 재면 덮어씁니다.
- 측정 환경이 달라지면(다른 장비, OS 교체) 비교가 무의미하므로 표 위에 환경을 적고,
  필요하면 새 기준선을 만듭니다.
- 절차와 릴리스 노트 표는 `docs/releasing.md`의 "성능 기준선과 표"를 봅니다.
