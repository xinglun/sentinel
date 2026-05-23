//! Sentinel の domain layer。
//!
//! 業務概念、値オブジェクト、エンティティ、純粋なドメインサービスだけを配置する。
//! IO、設定読み込み、report rendering、外部 API adapter へ依存してはならない。

pub mod evidence;
