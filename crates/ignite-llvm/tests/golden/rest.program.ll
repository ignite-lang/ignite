target triple = "x86_64-pc-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
; ModuleID = 'tests.rest'
source_filename = "tests.rest"

@ignite.empty = private constant [8 x i8] zeroinitializer, align 8
@.str.1 = private unnamed_addr constant [1 x i8] c"+", align 1
@.str.0 = private unnamed_addr constant [24 x i8] c"tests/golden/rest.ignite", align 1

declare void @ignite_rt_init(i32, ptr)
declare ptr @ignite_rt_alloc(i64, i64)
declare i8 @ignite_rt_spawn(ptr, ptr)
declare void @ignite_rt_spawn_ready()
declare void @ignite_rt_task_end()
declare i64 @ignite_rt_arena_mark()
declare void @ignite_rt_arena_release(i64)
declare i8 @ignite_rt_arena_due(i64, i64)
declare void @ignite_rt_arena_scratch(i8)
declare void @ignite_rt_arena_scratch_clear()
declare ptr @ignite_rt_arena_state() nounwind willreturn speculatable memory(none)
declare i32 @ignite_rt_args_port(ptr, ptr, ptr)
declare void @ignite_rt_exit(i32) noreturn
declare void @ignite_rt_fail(ptr) noreturn
declare void @ignite_rt_log(ptr)
declare i32 @ignite_rt_tcp_bind(ptr, i64, ptr, ptr)
declare i32 @ignite_rt_tcp_accept(i64, ptr, ptr)
declare i64 @ignite_rt_tcp_local_port(i64)
declare i32 @ignite_rt_tcp_connect(ptr, i64, ptr, ptr)
declare i32 @ignite_rt_tcp_read(i64, i64, ptr, ptr)
declare i32 @ignite_rt_tcp_write_all(i64, ptr, ptr)
declare i32 @ignite_rt_tcp_close(i64, ptr)
declare void @ignite_rt_tcp_release(i64)
declare i32 @ignite_rt_stdin_read(i64, ptr, ptr)
declare i32 @ignite_rt_stdin_is_terminal(ptr, ptr)
declare i32 @ignite_rt_stdin_set_raw(i8, ptr)
declare i32 @ignite_rt_stdout_write_all(ptr, ptr)
declare void @ignite_rt_io_error_new(ptr, ptr)
declare void @ignite_rt_text_from_bytes_lossy(ptr, ptr)
declare void @ignite_rt_text_trim(ptr, ptr)
declare void @ignite_rt_text_concat(ptr, ptr, ptr)
declare i8 @ignite_rt_text_eq(ptr, ptr)
declare i8 @ignite_rt_text_starts_with(ptr, ptr)
declare void @ignite_rt_text_join(ptr, ptr, ptr)
declare void @ignite_rt_int_to_text(ptr, i64)
declare i8 @ignite_rt_int_parse(ptr, ptr)
declare i32 @ignite_rt_tcp_set_idle_timeout(i64, i64, ptr)
declare ptr @ignite_rt_store_new(ptr, ptr, ptr, i64, i64)
declare i64 @ignite_rt_store_next_id(ptr)
declare void @ignite_rt_store_put(ptr, i64, ptr)
declare i8 @ignite_rt_store_get(ptr, i64, ptr)
declare i8 @ignite_rt_store_remove(ptr, i64, ptr)
declare void @ignite_rt_store_values(ptr, ptr)
declare i8 @ignite_rt_store_update(ptr, i64, ptr, ptr, ptr)
declare ptr @malloc(i64)
declare void @free(ptr)
declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)
declare void @llvm.memmove.p0.p0.i64(ptr, ptr, i64, i1)
declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64)
declare { i64, i1 } @llvm.ssub.with.overflow.i64(i64, i64)
declare { i64, i1 } @llvm.smul.with.overflow.i64(i64, i64)
declare { i8, i1 } @llvm.uadd.with.overflow.i8(i8, i8)
declare { i8, i1 } @llvm.usub.with.overflow.i8(i8, i8)
declare { i8, i1 } @llvm.umul.with.overflow.i8(i8, i8)
declare void @"ignite_rt_fail_arith"(ptr, ptr, i64, i64, i64, i64)
declare void @"ignite_rt_fail_match"(ptr, i64, i64)

define internal void @ignite.reserve(ptr %list, i64 %extra, i64 %size, i64 %align) {
entry:
  %lenp = getelementptr inbounds i8, ptr %list, i64 8
  %len = load i64, ptr %lenp, align 8
  %capp = getelementptr inbounds i8, ptr %list, i64 16
  %cap = load i64, ptr %capp, align 8
  %need = add i64 %len, %extra
  %fits = icmp sle i64 %need, %cap
  br i1 %fits, label %done, label %grow
grow:
  %double = mul i64 %cap, 2
  %big = icmp sgt i64 %double, %need
  %c1 = select i1 %big, i64 %double, i64 %need
  %small = icmp slt i64 %c1, 4
  %newcap = select i1 %small, i64 4, i64 %c1
  %bytes = mul i64 %newcap, %size
  %new = call ptr @ignite.alloc(i64 %bytes, i64 %align)
  %old = load ptr, ptr %list, align 8
  %used = mul i64 %len, %size
  call void @llvm.memcpy.p0.p0.i64(ptr %new, ptr %old, i64 %used, i1 false)
  store ptr %new, ptr %list, align 8
  store i64 %newcap, ptr %capp, align 8
  br label %done
done:
  ret void
}

define internal ptr @ignite.alloc(i64 %size, i64 %align) alwaysinline {
entry:
  %st = call ptr @ignite_rt_arena_state()
  %cur = load ptr, ptr %st, align 8
  %limp = getelementptr inbounds i8, ptr %st, i64 8
  %lim = load ptr, ptr %limp, align 8
  %ci = ptrtoint ptr %cur to i64
  %li = ptrtoint ptr %lim to i64
  %am1 = sub i64 %align, 1
  %up = add i64 %ci, %am1
  %mask = sub i64 0, %align
  %start = and i64 %up, %mask
  %end = add i64 %start, %size
  %small = icmp ult i64 %size, 65536
  %fits = icmp ule i64 %end, %li
  %live = icmp ne i64 %li, 0
  %fit = and i1 %fits, %live
  %ok = and i1 %small, %fit
  br i1 %ok, label %fast, label %slow
fast:
  %skip = sub i64 %start, %ci
  %p = getelementptr i8, ptr %cur, i64 %skip
  %next = getelementptr i8, ptr %p, i64 %size
  store ptr %next, ptr %st, align 8
  ret ptr %p
slow:
  %q = call ptr @ignite_rt_alloc(i64 %size, i64 %align)
  ret ptr %q
}

define internal i64 @ignite.arena_mark() alwaysinline {
entry:
  %st = call ptr @ignite_rt_arena_state()
  %cur = load ptr, ptr %st, align 8
  %offp = getelementptr inbounds i8, ptr %st, i64 16
  %off = load i64, ptr %offp, align 8
  %ci = ptrtoint ptr %cur to i64
  %pos = add i64 %ci, %off
  ret i64 %pos
}

define internal i1 @ignite.arena_due(i64 %mark, i64 %kept) alwaysinline {
entry:
  %pos = call i64 @ignite.arena_mark()
  %above = sub i64 %pos, %mark
  %four = shl i64 %kept, 2
  %big = icmp sgt i64 %four, 65536
  %limit = select i1 %big, i64 %four, i64 65536
  %due = icmp sgt i64 %above, %limit
  ret i1 %due
}

define internal void @ignite.arena_release(i64 %mark) alwaysinline {
entry:
  %st = call ptr @ignite_rt_arena_state()
  %cur = load ptr, ptr %st, align 8
  %offp = getelementptr inbounds i8, ptr %st, i64 16
  %off = load i64, ptr %offp, align 8
  %ci = ptrtoint ptr %cur to i64
  %pos = add i64 %ci, %off
  %nothing = icmp sge i64 %mark, %pos
  br i1 %nothing, label %done, label %down
down:
  %basep = getelementptr inbounds i8, ptr %st, i64 24
  %base = load ptr, ptr %basep, align 8
  %bi = ptrtoint ptr %base to i64
  %bpos = add i64 %bi, %off
  %inside = icmp sge i64 %mark, %bpos
  br i1 %inside, label %chunk, label %slow
chunk:
  %back = sub i64 %mark, %bpos
  %new = getelementptr i8, ptr %base, i64 %back
  store ptr %new, ptr %st, align 8
  br label %done
slow:
  call void @ignite_rt_arena_release(i64 %mark)
  br label %done
done:
  ret void
}

define i64 @"tests.rest.shape"(ptr %p0) !dbg !4 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t20 = alloca [24 x i8], align 8
  %t4 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !7
  %t5 = load i64, ptr %t4, align 8, !dbg !7
  %t6 = icmp eq i64 %t5, 0, !dbg !7
  br i1 %t6, label %L7, label %L3, !dbg !7
L7:
  store i64 0, ptr %t1, align 8, !dbg !7
  br label %L2, !dbg !7
L3:
  %t9 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !7
  %t10 = load i64, ptr %t9, align 8, !dbg !7
  %t11 = icmp eq i64 %t10, 1, !dbg !7
  br i1 %t11, label %L12, label %L8, !dbg !7
L12:
  %t13 = load ptr, ptr %p0, align 8, !dbg !7
  %t14 = load i64, ptr %t13, align 8, !dbg !7
  store i64 1, ptr %t1, align 8, !dbg !7
  br label %L2, !dbg !7
L8:
  %t16 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !7
  %t17 = load i64, ptr %t16, align 8, !dbg !7
  %t18 = icmp sge i64 %t17, 2, !dbg !7
  br i1 %t18, label %L19, label %L15, !dbg !7
L19:
  store i64 2, ptr %t1, align 8, !dbg !7
  br label %L2, !dbg !7
L15:
  store ptr @.str.0, ptr %t20, align 8, !dbg !7
  %t21 = getelementptr inbounds i8, ptr %t20, i64 8, !dbg !7
  store i64 24, ptr %t21, align 8, !dbg !7
  %t22 = getelementptr inbounds i8, ptr %t20, i64 16, !dbg !7
  store i64 24, ptr %t22, align 8, !dbg !7
  call void @ignite_rt_fail_match(ptr %t20, i64 4, i64 10), !dbg !7
  unreachable, !dbg !7
L2:
  %t23 = load i64, ptr %t1, align 8, !dbg !7
  ret i64 %t23, !dbg !6
}

define void @"tests.rest.tail"(ptr %sret, ptr %p0) !dbg !12 {
entry:
  %t1 = alloca [24 x i8], align 8
  %t11 = alloca [24 x i8], align 8
  %t14 = alloca [24 x i8], align 8
  %t20 = alloca [24 x i8], align 8
  %t23 = alloca [24 x i8], align 8
  %t4 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !15
  %t5 = load i64, ptr %t4, align 8, !dbg !15
  %t6 = icmp sge i64 %t5, 1, !dbg !15
  br i1 %t6, label %L7, label %L3, !dbg !15
L7:
  %t8 = load ptr, ptr %p0, align 8, !dbg !15
  %t9 = getelementptr inbounds i8, ptr %t8, i64 24, !dbg !15
  %t10 = sub i64 %t5, 1, !dbg !15
  store ptr %t9, ptr %t11, align 8, !dbg !15
  %t12 = getelementptr inbounds i8, ptr %t11, i64 8, !dbg !15
  store i64 %t10, ptr %t12, align 8, !dbg !15
  %t13 = getelementptr inbounds i8, ptr %t11, i64 16, !dbg !15
  store i64 %t10, ptr %t13, align 8, !dbg !15
  call void @"copy.List<Text>"(ptr %t14, ptr %t11), !dbg !15
  call void @"copy.List<Text>"(ptr %t1, ptr %t14), !dbg !15
  br label %L2, !dbg !15
L3:
  %t16 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !15
  %t17 = load i64, ptr %t16, align 8, !dbg !15
  %t18 = icmp eq i64 %t17, 0, !dbg !15
  br i1 %t18, label %L19, label %L15, !dbg !15
L19:
  store ptr @ignite.empty, ptr %t20, align 8, !dbg !18
  %t21 = getelementptr inbounds i8, ptr %t20, i64 8, !dbg !18
  store i64 0, ptr %t21, align 8, !dbg !18
  %t22 = getelementptr inbounds i8, ptr %t20, i64 16, !dbg !18
  store i64 0, ptr %t22, align 8, !dbg !18
  call void @llvm.memcpy.p0.p0.i64(ptr %t1, ptr %t20, i64 24, i1 false), !dbg !15
  br label %L2, !dbg !15
L15:
  store ptr @.str.0, ptr %t23, align 8, !dbg !15
  %t24 = getelementptr inbounds i8, ptr %t23, i64 8, !dbg !15
  store i64 24, ptr %t24, align 8, !dbg !15
  %t25 = getelementptr inbounds i8, ptr %t23, i64 16, !dbg !15
  store i64 24, ptr %t25, align 8, !dbg !15
  call void @ignite_rt_fail_match(ptr %t23, i64 12, i64 10), !dbg !15
  unreachable, !dbg !15
L2:
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t1, i64 24, i1 false), !dbg !14
  ret void, !dbg !14
}

define i64 @"tests.rest.ends"(ptr %p0) !dbg !19 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t10 = alloca [8 x i8], align 8
  %t15 = alloca [8 x i8], align 8
  %t22 = alloca [24 x i8], align 8
  %t25 = alloca [24 x i8], align 8
  %t36 = alloca [8 x i8], align 8
  %t4 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !22
  %t5 = load i64, ptr %t4, align 8, !dbg !22
  %t6 = icmp sge i64 %t5, 2, !dbg !22
  br i1 %t6, label %L7, label %L3, !dbg !22
L7:
  %t8 = load ptr, ptr %p0, align 8, !dbg !22
  %t9 = load i64, ptr %t8, align 8, !dbg !22
  store i64 %t9, ptr %t10, align 8, !dbg !22
  %t11 = sub i64 %t5, 1, !dbg !22
  %t12 = mul i64 %t11, 8, !dbg !22
  %t13 = getelementptr inbounds i8, ptr %t8, i64 %t12, !dbg !22
  %t14 = load i64, ptr %t13, align 8, !dbg !22
  store i64 %t14, ptr %t15, align 8, !dbg !22
  %t16 = load i64, ptr %t10, align 8, !dbg !24
  %t17 = load i64, ptr %t15, align 8, !dbg !25
  %t18 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %t16, i64 %t17), !dbg !24
  %t19 = extractvalue { i64, i1 } %t18, 1, !dbg !24
  br i1 %t19, label %L20, label %L21, !dbg !24
L20:
  store ptr @.str.0, ptr %t22, align 8, !dbg !24
  %t23 = getelementptr inbounds i8, ptr %t22, i64 8, !dbg !24
  store i64 24, ptr %t23, align 8, !dbg !24
  %t24 = getelementptr inbounds i8, ptr %t22, i64 16, !dbg !24
  store i64 24, ptr %t24, align 8, !dbg !24
  store ptr @.str.1, ptr %t25, align 8, !dbg !24
  %t26 = getelementptr inbounds i8, ptr %t25, i64 8, !dbg !24
  store i64 1, ptr %t26, align 8, !dbg !24
  %t27 = getelementptr inbounds i8, ptr %t25, i64 16, !dbg !24
  store i64 1, ptr %t27, align 8, !dbg !24
  call void @ignite_rt_fail_arith(ptr %t25, ptr %t22, i64 20, i64 19, i64 %t16, i64 %t17), !dbg !24
  unreachable, !dbg !24
L21:
  %t28 = extractvalue { i64, i1 } %t18, 0, !dbg !24
  store i64 %t28, ptr %t1, align 8, !dbg !22
  br label %L2, !dbg !22
L3:
  %t30 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !22
  %t31 = load i64, ptr %t30, align 8, !dbg !22
  %t32 = icmp eq i64 %t31, 1, !dbg !22
  br i1 %t32, label %L33, label %L29, !dbg !22
L33:
  %t34 = load ptr, ptr %p0, align 8, !dbg !22
  %t35 = load i64, ptr %t34, align 8, !dbg !22
  store i64 %t35, ptr %t36, align 8, !dbg !22
  %t37 = load i64, ptr %t36, align 8, !dbg !26
  store i64 %t37, ptr %t1, align 8, !dbg !22
  br label %L2, !dbg !22
L29:
  %t39 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !22
  %t40 = load i64, ptr %t39, align 8, !dbg !22
  %t41 = icmp eq i64 %t40, 0, !dbg !22
  br i1 %t41, label %L42, label %L38, !dbg !22
L42:
  store i64 0, ptr %t1, align 8, !dbg !22
  br label %L2, !dbg !22
L38:
  store ptr @.str.0, ptr %t25, align 8, !dbg !22
  %t43 = getelementptr inbounds i8, ptr %t25, i64 8, !dbg !22
  store i64 24, ptr %t43, align 8, !dbg !22
  %t44 = getelementptr inbounds i8, ptr %t25, i64 16, !dbg !22
  store i64 24, ptr %t44, align 8, !dbg !22
  call void @ignite_rt_fail_match(ptr %t25, i64 19, i64 10), !dbg !22
  unreachable, !dbg !22
L2:
  %t45 = load i64, ptr %t1, align 8, !dbg !22
  ret i64 %t45, !dbg !21
}

define void @"tests.rest.after_command"(ptr %sret, ptr %p0) !dbg !28 {
entry:
  %t1 = alloca [24 x i8], align 8
  %t14 = alloca [24 x i8], align 8
  %t17 = alloca [24 x i8], align 8
  %t19 = alloca [24 x i8], align 8
  %t4 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !31
  %t5 = load i64, ptr %t4, align 8, !dbg !31
  %t6 = icmp sge i64 %t5, 2, !dbg !31
  br i1 %t6, label %L7, label %L3, !dbg !31
L7:
  %t8 = load ptr, ptr %p0, align 8, !dbg !31
  %t9 = load i8, ptr %t8, align 1, !dbg !31
  %t10 = icmp eq i8 %t9, -1, !dbg !31
  br i1 %t10, label %L11, label %L3, !dbg !31
L11:
  %t12 = getelementptr inbounds i8, ptr %t8, i64 2, !dbg !31
  %t13 = sub i64 %t5, 2, !dbg !31
  store ptr %t12, ptr %t14, align 8, !dbg !31
  %t15 = getelementptr inbounds i8, ptr %t14, i64 8, !dbg !31
  store i64 %t13, ptr %t15, align 8, !dbg !31
  %t16 = getelementptr inbounds i8, ptr %t14, i64 16, !dbg !31
  store i64 %t13, ptr %t16, align 8, !dbg !31
  call void @"copy.List<Byte>"(ptr %t17, ptr %t14), !dbg !31
  call void @"copy.List<Byte>"(ptr %t1, ptr %t17), !dbg !31
  br label %L2, !dbg !31
L3:
  call void @"copy.List<Byte>"(ptr %t1, ptr %p0), !dbg !31
  br label %L2, !dbg !31
L18:
  store ptr @.str.0, ptr %t19, align 8, !dbg !31
  %t20 = getelementptr inbounds i8, ptr %t19, i64 8, !dbg !31
  store i64 24, ptr %t20, align 8, !dbg !31
  %t21 = getelementptr inbounds i8, ptr %t19, i64 16, !dbg !31
  store i64 24, ptr %t21, align 8, !dbg !31
  call void @ignite_rt_fail_match(ptr %t19, i64 27, i64 10), !dbg !31
  unreachable, !dbg !31
L2:
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t1, i64 24, i1 false), !dbg !30
  ret void, !dbg !30
}

define i64 @"tests.rest.status"(ptr %p0) !dbg !35 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t13 = alloca [24 x i8], align 8
  %t4 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !38
  %t5 = load i64, ptr %t4, align 8, !dbg !38
  %t6 = icmp sge i64 %t5, 2, !dbg !38
  br i1 %t6, label %L7, label %L3, !dbg !38
L7:
  %t8 = getelementptr inbounds i8, ptr %p0, i64 24, !dbg !38
  %t9 = load i64, ptr %t8, align 8, !dbg !38
  store i64 400, ptr %t1, align 8, !dbg !38
  br label %L2, !dbg !38
L3:
  %t11 = getelementptr inbounds i8, ptr %p0, i64 24, !dbg !38
  %t12 = load i64, ptr %t11, align 8, !dbg !38
  store i64 200, ptr %t1, align 8, !dbg !38
  br label %L2, !dbg !38
L10:
  store ptr @.str.0, ptr %t13, align 8, !dbg !38
  %t14 = getelementptr inbounds i8, ptr %t13, i64 8, !dbg !38
  store i64 24, ptr %t14, align 8, !dbg !38
  %t15 = getelementptr inbounds i8, ptr %t13, i64 16, !dbg !38
  store i64 24, ptr %t15, align 8, !dbg !38
  call void @ignite_rt_fail_match(ptr %t13, i64 34, i64 10), !dbg !38
  unreachable, !dbg !38
L2:
  %t16 = load i64, ptr %t1, align 8, !dbg !38
  ret i64 %t16, !dbg !37
}

define internal void @"copy.List<Byte>"(ptr %dst, ptr %src) {
entry:
  call void @llvm.memcpy.p0.p0.i64(ptr %dst, ptr %src, i64 24, i1 false)
  %t1 = getelementptr inbounds i8, ptr %src, i64 8
  %t2 = load i64, ptr %t1, align 8
  %t3 = load ptr, ptr %src, align 8
  %t4 = icmp sle i64 %t2, 0
  br i1 %t4, label %L5, label %L6
L5:
  store ptr @ignite.empty, ptr %dst, align 8
  %t8 = getelementptr inbounds i8, ptr %dst, i64 8
  store i64 0, ptr %t8, align 8
  %t9 = getelementptr inbounds i8, ptr %dst, i64 16
  store i64 0, ptr %t9, align 8
  br label %L7
L6:
  %t10 = mul i64 %t2, 1
  %t11 = call ptr @ignite.alloc(i64 %t10, i64 1)
  call void @llvm.memcpy.p0.p0.i64(ptr %t11, ptr %t3, i64 %t10, i1 false)
  store ptr %t11, ptr %dst, align 8
  %t12 = getelementptr inbounds i8, ptr %dst, i64 8
  store i64 %t2, ptr %t12, align 8
  %t13 = getelementptr inbounds i8, ptr %dst, i64 16
  store i64 %t2, ptr %t13, align 8
  br label %L7
L7:
  ret void
}

define internal void @"copy.List<Text>"(ptr %dst, ptr %src) {
entry:
  %t12 = alloca [8 x i8], align 8
  call void @llvm.memcpy.p0.p0.i64(ptr %dst, ptr %src, i64 24, i1 false)
  %t1 = getelementptr inbounds i8, ptr %src, i64 8
  %t2 = load i64, ptr %t1, align 8
  %t3 = load ptr, ptr %src, align 8
  %t4 = icmp sle i64 %t2, 0
  br i1 %t4, label %L5, label %L6
L5:
  store ptr @ignite.empty, ptr %dst, align 8
  %t8 = getelementptr inbounds i8, ptr %dst, i64 8
  store i64 0, ptr %t8, align 8
  %t9 = getelementptr inbounds i8, ptr %dst, i64 16
  store i64 0, ptr %t9, align 8
  br label %L7
L6:
  %t10 = mul i64 %t2, 24
  %t11 = call ptr @ignite.alloc(i64 %t10, i64 8)
  call void @llvm.memcpy.p0.p0.i64(ptr %t11, ptr %t3, i64 %t10, i1 false)
  store i64 0, ptr %t12, align 8
  br label %L13
L13:
  %t16 = load i64, ptr %t12, align 8
  %t17 = icmp slt i64 %t16, %t2
  br i1 %t17, label %L14, label %L15
L14:
  %t18 = mul i64 %t16, 24
  %t19 = getelementptr inbounds i8, ptr %t11, i64 %t18
  %t20 = getelementptr inbounds i8, ptr %t3, i64 %t18
  call void @"copy.Text"(ptr %t19, ptr %t20)
  %t21 = add i64 %t16, 1
  store i64 %t21, ptr %t12, align 8
  br label %L13
L15:
  store ptr %t11, ptr %dst, align 8
  %t22 = getelementptr inbounds i8, ptr %dst, i64 8
  store i64 %t2, ptr %t22, align 8
  %t23 = getelementptr inbounds i8, ptr %dst, i64 16
  store i64 %t2, ptr %t23, align 8
  br label %L7
L7:
  ret void
}

define internal void @"copy.Text"(ptr %dst, ptr %src) {
entry:
  call void @llvm.memcpy.p0.p0.i64(ptr %dst, ptr %src, i64 24, i1 false)
  %t1 = getelementptr inbounds i8, ptr %src, i64 8
  %t2 = load i64, ptr %t1, align 8
  %t3 = load ptr, ptr %src, align 8
  %t4 = icmp sle i64 %t2, 0
  br i1 %t4, label %L5, label %L6
L5:
  store ptr @ignite.empty, ptr %dst, align 8
  %t8 = getelementptr inbounds i8, ptr %dst, i64 8
  store i64 0, ptr %t8, align 8
  %t9 = getelementptr inbounds i8, ptr %dst, i64 16
  store i64 0, ptr %t9, align 8
  br label %L7
L6:
  %t10 = mul i64 %t2, 1
  %t11 = call ptr @ignite.alloc(i64 %t10, i64 1)
  call void @llvm.memcpy.p0.p0.i64(ptr %t11, ptr %t3, i64 %t10, i1 false)
  store ptr %t11, ptr %dst, align 8
  %t12 = getelementptr inbounds i8, ptr %dst, i64 8
  store i64 %t2, ptr %t12, align 8
  %t13 = getelementptr inbounds i8, ptr %dst, i64 16
  store i64 %t2, ptr %t13, align 8
  br label %L7
L7:
  ret void
}

!llvm.dbg.cu = !{!1}
!llvm.module.flags = !{!42, !43}
!0 = !DIFile(filename: "tests.rest.ll", directory: ".")
!1 = distinct !DICompileUnit(language: DW_LANG_C, file: !0, producer: "ignite", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug, nameTableKind: None)
!2 = !DISubroutineType(types: !{})
!3 = !DIFile(filename: "rest.ignite", directory: "./tests/golden")
!4 = distinct !DISubprogram(name: "tests.rest.shape", linkageName: "tests.rest.shape", scope: !3, file: !3, line: 3, type: !2, scopeLine: 3, spFlags: DISPFlagDefinition, unit: !1)
!5 = !DILocation(line: 3, column: 32, scope: !4)
!6 = !DILocation(line: 4, column: 3, scope: !4)
!7 = !DILocation(line: 4, column: 10, scope: !4)
!8 = !DILocation(line: 4, column: 16, scope: !4)
!9 = !DILocation(line: 5, column: 11, scope: !4)
!10 = !DILocation(line: 6, column: 12, scope: !4)
!11 = !DILocation(line: 7, column: 19, scope: !4)
!12 = distinct !DISubprogram(name: "tests.rest.tail", linkageName: "tests.rest.tail", scope: !3, file: !3, line: 11, type: !2, scopeLine: 11, spFlags: DISPFlagDefinition, unit: !1)
!13 = !DILocation(line: 11, column: 39, scope: !12)
!14 = !DILocation(line: 12, column: 3, scope: !12)
!15 = !DILocation(line: 12, column: 10, scope: !12)
!16 = !DILocation(line: 12, column: 16, scope: !12)
!17 = !DILocation(line: 13, column: 20, scope: !12)
!18 = !DILocation(line: 14, column: 11, scope: !12)
!19 = distinct !DISubprogram(name: "tests.rest.ends", linkageName: "tests.rest.ends", scope: !3, file: !3, line: 18, type: !2, scopeLine: 18, spFlags: DISPFlagDefinition, unit: !1)
!20 = !DILocation(line: 18, column: 31, scope: !19)
!21 = !DILocation(line: 19, column: 3, scope: !19)
!22 = !DILocation(line: 19, column: 10, scope: !19)
!23 = !DILocation(line: 19, column: 16, scope: !19)
!24 = !DILocation(line: 20, column: 19, scope: !19)
!25 = !DILocation(line: 20, column: 23, scope: !19)
!26 = !DILocation(line: 21, column: 12, scope: !19)
!27 = !DILocation(line: 22, column: 11, scope: !19)
!28 = distinct !DISubprogram(name: "tests.rest.after_command", linkageName: "tests.rest.after_command", scope: !3, file: !3, line: 26, type: !2, scopeLine: 26, spFlags: DISPFlagDefinition, unit: !1)
!29 = !DILocation(line: 26, column: 41, scope: !28)
!30 = !DILocation(line: 27, column: 3, scope: !28)
!31 = !DILocation(line: 27, column: 10, scope: !28)
!32 = !DILocation(line: 27, column: 16, scope: !28)
!33 = !DILocation(line: 28, column: 25, scope: !28)
!34 = !DILocation(line: 29, column: 10, scope: !28)
!35 = distinct !DISubprogram(name: "tests.rest.status", linkageName: "tests.rest.status", scope: !3, file: !3, line: 33, type: !2, scopeLine: 33, spFlags: DISPFlagDefinition, unit: !1)
!36 = !DILocation(line: 33, column: 46, scope: !35)
!37 = !DILocation(line: 34, column: 3, scope: !35)
!38 = !DILocation(line: 34, column: 10, scope: !35)
!39 = !DILocation(line: 34, column: 16, scope: !35)
!40 = !DILocation(line: 35, column: 24, scope: !35)
!41 = !DILocation(line: 36, column: 15, scope: !35)
!42 = !{i32 7, !"Dwarf Version", i32 5}
!43 = !{i32 2, !"Debug Info Version", i32 3}
