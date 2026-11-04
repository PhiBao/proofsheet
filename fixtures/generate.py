#!/usr/bin/env python3
"""Deterministic generator for Proofsheet's demo fixtures.

The dirt is planted on purpose, but every file is generated from a fixed seed so
the demo, the README, and the containment claims are reproducible.
"""
import csv
import random
from datetime import date, timedelta

random.seed(20261104)

MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]


def messy_date(d: date, style: int) -> str:
    if style == 0:
        return d.strftime("%Y-%m-%d")
    if style == 1:
        return d.strftime("%m/%d/%Y")
    if style == 2:
        return f"{MONTHS[d.month - 1]} {d.day}, {d.year}"
    if style == 3:
        return d.strftime("%Y/%m/%d")
    return d.strftime("%d-%m-%Y")


def money(v: float, style: int) -> str:
    if style == 0:
        return f"${v:,.2f}"
    if style == 1:
        return f"{v:.2f}"
    if style == 2:
        return f"${v:,.0f}"
    return f"{v:,.2f}"


products = [
    ("Aurora Desk Lamp", 89.0),
    ("Field Notebook", 14.5),
    ("Cast Iron Pan", 68.0),
    ("Trail Backpack", 129.0),
    ("Wool Throw", 74.0),
    ("Ceramic Mug", 18.0),
    ("Oak Stool", 156.0),
    ("Linen Apron", 42.0),
    ("Copper Kettle", 98.0),
    ("Canvas Tote", 32.0),
]

countries = [
    ("United States", 0.52),
    ("Canada", 0.12),
    ("Germany", 0.10),
    ("United Kingdom", 0.09),
    ("Australia", 0.07),
    ("France", 0.05),
    ("Netherlands", 0.05),
]

first = ["Ada", "Miles", "Noor", "Jonas", "Priya", "Elena", "Theo", "Maya", "Owen", "Sofia", "Iris", "Hugo"]
last = ["Bennett", "Okafor", "Lindqvist", "Moreau", "Silva", "Haddad", "Nakamura", "Fischer", "Kowalski", "Duarte"]


def pick_country() -> str:
    r = random.random()
    acc = 0.0
    base = "United States"
    for name, w in countries:
        acc += w
        if r <= acc:
            base = name
            break
    style = random.random()
    if style < 0.62:
        return base
    if style < 0.76:
        return base.lower()
    if style < 0.86:
        return base.upper()
    if style < 0.94:
        return base + " "
    return base + "."


def make_orders(path: str) -> None:
    rows = []
    start = date(2024, 1, 1)
    for i in range(506):
        order_id = f"NW-{10001 + i}"
        d = start + timedelta(days=random.randint(0, 364))
        name = f"{random.choice(first)} {random.choice(last)}"
        email = f"{name.lower().replace(' ', '.')}@{random.choice(['example.com', 'mail.test', 'acme.co'])}"
        if random.random() < 0.08:
            email = " " + email.upper() + " "
        if random.random() < 0.06:
            name = name + "  "
        product, base_price = random.choice(products)
        qty = random.choice([1, 1, 1, 2, 2, 3, 4, 5])
        qty_raw = str(qty)
        if random.random() < 0.02:
            qty_raw = f"{qty}x"
        elif random.random() < 0.01:
            qty_raw = ""
        price = base_price * random.choice([1.0, 1.0, 1.0, 1.05, 0.95])
        price_raw = money(price, random.choice([0, 0, 0, 1, 2]))
        discount = random.choice([0.0, 0.0, 0.0, 0.05, 0.1, 0.15])
        discount_raw = random.choice(
            [f"{discount:.2f}", f"{int(discount * 100)}%", ""]
        )
        status = random.choice(
            ["completed", "completed", "completed", "shipped", "shipped", "cancelled", "refunded"]
        )
        status = random.choice([status, status.capitalize(), status.upper()]) if random.random() < 0.18 else status
        ship = random.choice([0.0, 4.95, 7.5, 12.0, 0.0])
        ship_raw = "" if random.random() < 0.09 else money(ship, random.choice([0, 1]))
        rows.append(
            [
                order_id,
                messy_date(d, random.choice([0, 1, 2, 3, 4])),
                name,
                email,
                pick_country(),
                product,
                qty_raw,
                price_raw,
                discount_raw,
                status,
                ship_raw,
            ]
        )

    # Exact duplicates — the most common export bug.
    for _ in range(16):
        rows.append(list(random.choice(rows)))

    random.shuffle(rows)

    with open(path, "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(
            [
                "order_id",
                "order_date",
                "customer_name",
                "email",
                "country",
                "product",
                "quantity",
                "unit_price",
                "discount",
                "status",
                "shipping_cost",
            ]
        )
        w.writerows(rows)


def make_saas(path: str) -> None:
    rows = []
    regions = [("North America", 62000), ("Europe", 38000), ("APAC", 21000)]
    for m in range(24):
        year = 2024 + m // 12
        month = (m % 12) + 1
        d = date(year, month, 1)
        month_styles = [d.strftime("%Y-%m"), d.strftime("%b %Y"), d.strftime("%m/%Y"), d.strftime("%Y/%m")]
        for region, base in regions:
            growth = 1 + 0.021 * m
            mrr = base * growth * random.uniform(0.96, 1.04)
            active = int(mrr / random.uniform(48, 62))
            new_c = random.randint(6, 34)
            churned = random.randint(1, 12)
            churn = churned / max(active, 1)
            mrr_raw = money(mrr, random.choice([0, 0, 0, 1, 2]))
            if random.random() < 0.04:
                mrr_raw = ""
            churn_raw = random.choice(
                [f"{churn * 100:.2f}%", f"{churn:.4f}", f"{churn * 100:.1f} %"]
            )
            if random.random() < 0.03:
                churn_raw = ""
            region_raw = region
            if random.random() < 0.22:
                region_raw = random.choice([region.lower(), region.upper(), region + " "])
            rows.append(
                [
                    random.choice(month_styles),
                    region_raw,
                    mrr_raw,
                    new_c,
                    churned,
                    churn_raw,
                    active,
                ]
            )

    for _ in range(4):
        rows.append(list(random.choice(rows)))
    random.shuffle(rows)

    with open(path, "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(
            [
                "month",
                "region",
                "mrr",
                "new_customers",
                "churned_customers",
                "churn_rate",
                "active_customers",
            ]
        )
        w.writerows(rows)


if __name__ == "__main__":
    make_orders("northwind_orders_messy.csv")
    make_saas("atlas_saas_metrics_messy.csv")
    print("fixtures written")
