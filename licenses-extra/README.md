# licenses-extra

게시된 crates.io 패키지에 LICENSE 파일이 없는 크레이트의 업스트림 저장소 LICENSE 사본.
`scripts/generate-license-data.mjs`의 `EXTRA_LICENSES`가 이 파일을 cargo-about의 표준 템플릿
("Copyright (c) <year> <owner>") 대신 쓴다. 새 크레이트가 같은 상황이면 저장소의 LICENSE를
여기에 복사하고 스크립트에 한 줄 추가한다.
