-- Add cutoff_hours to pickup_points and delivery_zones so fulfillment cutoff
-- can be configured per location/zone, not just per inventory item.
-- Resolution precedence in booking service: zone > pickup_point > item.
ALTER TABLE pickup_points ADD COLUMN cutoff_hours INTEGER;
ALTER TABLE delivery_zones ADD COLUMN cutoff_hours INTEGER;
